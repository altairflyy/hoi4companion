import copy
import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from hoi4companion.guide_frames import GuideFrames, frame_reference, media_references

URL='https://www.youtube.com/watch?v=abcdefghijk'
OTHER='https://www.youtube.com/watch?v=lmnopqrstuv'


def fixture():
    step={'key':'build','action':{'kind':'manual','params':{}},'evidence':[
        {'source_id':'a','basis':'explicit','timestamp_seconds':101,'quote':'narration'},
        {'source_id':'b','basis':'visual','timestamp_seconds':44,'quote':'other video'}]}
    guide={'sources':[{'id':'a','type':'youtube','url':URL,'title':'Video one'},
                     {'id':'b','type':'youtube','url':OTHER,'title':'Video two'}],
           'source_inventory':{'schema_version':1,'items':[
               {'id':'v1','source_id':'a','basis':'visual','timestamp_seconds':121,'step_keys':['build'],'quote':'frame'},
               {'id':'v2','source_id':'a','basis':'visual','timestamp_seconds':130,'step_keys':['build']},
               {'id':'unused','source_id':'a','basis':'visual','timestamp_seconds':139,'step_keys':['other']},
               {'id':'excluded','source_id':'a','basis':'visual','timestamp_seconds':140,'step_keys':['build'],'excluded_reason':'unused'}]}}
    return guide,step


class MediaReferencesTests(unittest.TestCase):
    def test_distinct_sources_and_visual_evidence_remain_separate_from_narration(self):
        guide,step=fixture();before=copy.deepcopy((guide,step))
        result=media_references(guide,step)
        self.assertEqual([(x['basis'],x['url'],x['seconds']) for x in result['references']],
                         [('visual',OTHER,44),('visual',URL,121),('visual',URL,130),('explicit',URL,101)])
        self.assertEqual(len({x['id'] for x in result['references']}),4)
        self.assertEqual(result['references'][0]['clip_start'],39)
        self.assertEqual(result['references'][0]['clip_end'],54)
        self.assertEqual((guide,step),before)
        self.assertEqual(media_references(guide,step),result)

    def test_single_video_default_matches_legacy_selection_and_has_bounded_zero_context(self):
        guide,step=fixture();guide['sources']=guide['sources'][:1];step['evidence']=step['evidence'][:1]
        result=media_references(guide,step);ref=next(x for x in result['references'] if x['id']==result['default_reference'])
        self.assertEqual((ref['url'],ref['seconds']),(URL,121));self.assertEqual(frame_reference(guide,step)['seconds'],121)
        step['action']['params']['source_seconds']=0
        result=media_references(guide,step);ref=next(x for x in result['references'] if x['id']==result['default_reference'])
        self.assertEqual((ref['seconds'],ref['clip_start'],ref['clip_end']),(0,0,10))

    def test_ambiguous_unspecified_video_is_not_guessed(self):
        guide,step=fixture();step['action']['params']['source_seconds']=17
        result=media_references(guide,step)
        self.assertIsNone(result['default_reference'])
        self.assertFalse(any(x['basis']=='declared' and x['seconds']==17 for x in result['references']))

    def test_unrelated_untagged_web_source_is_not_attached_to_a_single_video(self):
        guide,step=fixture();guide['sources']=[guide['sources'][0],{'id':'b','type':'web','title':'Website'}]
        step['evidence']=[{'source_id':'b','basis':'explicit','timestamp_seconds':44,'quote':'web text'}]
        guide.pop('source_inventory')
        self.assertEqual(media_references(guide,step)['references'],[])

    def test_same_spoken_quote_is_not_duplicated_by_untagged_inventory(self):
        guide,step=fixture();guide['source_inventory']['items'].append(
            {'id':'repeat','source_id':'a','timestamp_seconds':101,'step_keys':['build'],'quote':'narration'})
        refs=media_references(guide,step)['references']
        self.assertEqual(len([r for r in refs if r['seconds']==101]),1)
        self.assertEqual(next(r for r in refs if r['seconds']==101)['basis'],'explicit')

    def test_unknown_source_id_does_not_borrow_the_only_video(self):
        guide,step=fixture();guide['sources']=guide['sources'][:1]
        guide.pop('source_inventory')
        step['evidence']=[{'source_id':'missing','basis':'visual','timestamp_seconds':44}]
        self.assertEqual(media_references(guide,step)['references'],[])
        step['action']['params']['source_seconds']=44
        self.assertEqual(media_references(guide,step)['references'],[])

    def test_clip_window_is_bounded_cached_and_uses_only_registered_video(self):
        with TemporaryDirectory() as d:
            root=Path(d);source=root/'source.mp4';source.write_bytes(b'video')
            frames=GuideFrames(root/'frames');frames.register(URL,source)
            calls=[]
            def render(args,**kwargs):calls.append(args);Path(args[-1]).write_bytes(b'clip')
            with patch('hoi4companion.guide_frames.ffmpeg_path',return_value='ffmpeg'),patch('hoi4companion.guide_frames.duration',return_value=125),patch('hoi4companion.guide_frames.command',side_effect=render):
                path=frames.clip(URL,121);self.assertEqual(path.read_bytes(),b'clip')
                self.assertEqual(frames.clip(URL,121),path);self.assertEqual(len(calls),1)
                self.assertEqual(calls[0][calls[0].index('-ss')+1],'116')
                self.assertEqual(calls[0][calls[0].index('-t')+1],'9')
                with self.assertRaises(ValueError):frames.clip(OTHER,20)
                for invalid in [True,-1,float('nan'),14400,126]:
                    with self.assertRaises(ValueError):frames.clip(URL,invalid)

    def test_clip_timeout_is_reported_and_temporary_file_removed(self):
        with TemporaryDirectory() as d:
            source=Path(d)/'source.mp4';source.write_bytes(b'video')
            frames=GuideFrames(Path(d)/'frames');frames.register(URL,source)
            def timeout(args,**kwargs):
                Path(args[-1]).write_bytes(b'partial')
                raise subprocess.TimeoutExpired(args,90)
            with patch('hoi4companion.guide_frames.ffmpeg_path',return_value='ffmpeg'),patch('hoi4companion.guide_frames.duration',return_value=125),patch('hoi4companion.guide_frames.command',side_effect=timeout):
                with self.assertRaisesRegex(ValueError,'tempo massimo'):frames.clip(URL,121)
            self.assertEqual(list(frames.root.glob('clip-*.mp4')),[])

    def test_http_media_ref_and_clip_use_only_reference_ids_and_preserve_default(self):
        from http.server import ThreadingHTTPServer
        from urllib.request import urlopen,Request
        from urllib.error import HTTPError
        import threading
        from hoi4companion.api import make_handler
        guide,step=fixture();guide['tracks']={'operations':[step]}
        with TemporaryDirectory() as d:
            image=Path(d)/'frame.jpg';image.write_bytes(b'abcdef')
            clip=Path(d)/'clip.mp4';clip.write_bytes(b'0123456789')
            frames=SimpleNamespace(frame=Mock(return_value=image),clip=Mock(return_value=clip),source=Mock(return_value=clip),metadata=Mock(return_value={'duration':300}))
            store=SimpleNamespace(walkthrough=lambda _:guide,guide_frames=frames)
            with ThreadingHTTPServer(('127.0.0.1',0),make_handler(store)) as server:
                worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start()
                base=f'http://127.0.0.1:{server.server_port}/api/guide/'
                try:
                    media=json.load(urlopen(base+'media?id=g&key=build'))
                    target=next(x for x in media['references'] if x['url']==URL and x['seconds']==121)
                    ref=target['id']
                    with urlopen(base+'frame?id=g&key=build&ref='+ref) as response:self.assertEqual(response.read(),b'abcdef')
                    frames.frame.assert_called_once_with(URL,121)
                    with urlopen(Request(base+'clip?id=g&key=build&ref='+ref,headers={'Range':'bytes=2-5'})) as response:
                        self.assertEqual(response.status,206);self.assertEqual(response.read(),b'2345');self.assertEqual(response.headers['Content-Range'],'bytes 2-5/10')
                    frames.clip.assert_called_once_with(URL,121)
                    for header in ['bytes=99-','bytes=5-2','bytes=0-1,3-5','bytes=-0']:
                        with self.assertRaises(HTTPError) as error:urlopen(Request(base+'clip?id=g&key=build&ref='+ref,headers={'Range':header}))
                        self.assertEqual(error.exception.code,416)
                    for suffix in ['media?id=g&key=absent','frame?id=g&key=build&ref=bad','clip?id=g&key=build&ref=bad','frame?id=g&key=build&ref=']:
                        with self.assertRaises(HTTPError):urlopen(base+suffix)
                finally:server.shutdown();worker.join(timeout=3)
