"""Native media source/time identity preserves the existing reference policy."""
import unittest
from pathlib import Path
from copy import deepcopy
from hoi4companion.backend_core import _request
from hoi4companion.guide_frames import frame_reference,media_references
from test_guide_frames import case,URL,OTHER

class NativeMediaReferenceTests(unittest.TestCase):
    def test_frame_and_reference_policies_keep_time_source_and_uncertainty(self):
        for variation in ('normal','zero','explicit','missing','ambiguous','web','unknown','quoted','duplicates','truncated','unicode','invalid-port'):
            guide,step=case()
            if variation=='zero':step['action']['params']['source_seconds']=0
            if variation=='explicit':step['action']['params']['source_seconds']=530
            if variation=='missing':guide.pop('source_inventory');step['evidence']=[]
            if variation=='ambiguous':guide['sources'].append({'id':'other','url':OTHER});guide['source_inventory']['items'].append({'id':'b','source_id':'other','basis':'visual','timestamp_seconds':600,'step_keys':['design']})
            if variation=='web':guide['sources'][0].update(type='web',url='https://example.com')
            if variation=='unknown':step['evidence'][0]['source_id']='absent';step['action']['params']['source_seconds']=30
            if variation=='quoted':step['evidence'][0].update(quote='Build a factory',end_seconds=500)
            if variation=='duplicates':guide['source_inventory']['items'].append({'id':'same',**step['evidence'][0],'step_keys':['design']})
            if variation=='truncated':step['evidence']=[{'source_id':'video','basis':'visual','timestamp_seconds':n}for n in range(300)]
            if variation=='unicode':guide['sources'][0]['title']='Video ünicode';step['evidence'][0]['quote']='é'*4100
            if variation=='invalid-port':guide['sources'][0]['url']='https://www.youtube.com:443/watch?v=abcdefghijk'
            before=deepcopy((guide,step))
            with self.subTest(variation=variation):
                self.assertEqual(_request(0,'guide_frame_reference',guide=guide,step=step),frame_reference(guide,step))
                self.assertEqual(_request(0,'guide_media_references',guide=guide,step=step),media_references(guide,step))
                self.assertEqual((guide,step),before)

    def test_youtube_canonicalization_does_not_accept_credentials_ports_or_playlists(self):
        from hoi4companion.import_sources import youtube_url
        for url in (URL,'https://youtu.be/abcdefghijk?t=40','https://m.youtube.com/shorts/abcdefghijk','https://youtube.com/live/abcdefghijk'):
            self.assertEqual(_request(0,'source_youtube_url',url=url),youtube_url(url))
        for url in ('http://youtu.be/abcdefghijk','https://user@youtube.com/watch?v=abcdefghijk','https://youtube.com:443/watch?v=abcdefghijk','https://youtube.com/playlist?list=test','https://youtube.com/watch?v=short'):
            with self.assertRaises(ValueError):_request(0,'source_youtube_url',url=url)

    def test_actual_ffmpeg_frame_clip_cache_and_unavailable_sources(self):
        import json,subprocess
        from tempfile import TemporaryDirectory
        from imageio_ffmpeg import get_ffmpeg_exe
        from PIL import Image
        ffmpeg=get_ffmpeg_exe()
        with TemporaryDirectory() as folder:
            root=Path(folder);source=root/'source.mp4'
            subprocess.run([ffmpeg,'-v','error','-nostdin','-f','lavfi','-i','color=c=blue:s=160x90:r=15','-t','2','-c:v','libx264','-y',str(source)],check=True,capture_output=True,timeout=25)
            cache=root/'frames';cache.mkdir();(cache/'sources.json').write_text(json.dumps({URL:{'path':str(source),'duration':2}}),encoding='utf8')
            args={'root':str(cache),'ffmpeg':ffmpeg,'url':URL}
            before=source.read_bytes()
            frame=Path(_request(0,'media_preview',**args,seconds=.5,clip=False))
            with Image.open(frame)as image:self.assertEqual(image.size,(160,90));self.assertEqual(image.format,'JPEG')
            stamp=frame.stat().st_mtime_ns
            self.assertEqual(_request(0,'media_preview',**args,seconds=.5,clip=False),str(frame));self.assertEqual(frame.stat().st_mtime_ns,stamp)
            clip=Path(_request(0,'media_preview',**args,seconds=.5,clip=True));self.assertGreater(clip.stat().st_size,0)
            for seconds in (-1,2,14400):
                with self.assertRaises(ValueError):_request(0,'media_preview',**args,seconds=seconds,clip=False)
            with self.assertRaises(ValueError):_request(0,'media_preview',**{**args,'url':OTHER},seconds=0,clip=False)
            self.assertEqual(source.read_bytes(),before);self.assertFalse(list(cache.glob('*-tmp-*')))


    def test_native_video_preset_and_real_keyframe_split_preserve_overlap_and_cap(self):
        import subprocess
        from tempfile import TemporaryDirectory
        from imageio_ffmpeg import get_ffmpeg_exe
        from hoi4companion.guide_media import ENCODING_PROFILE
        self.assertEqual(_request(0,'video_profile'),ENCODING_PROFILE)
        ffmpeg=get_ffmpeg_exe()
        with TemporaryDirectory() as folder:
            root=Path(folder);master=root/'master.mp4'
            subprocess.run([ffmpeg,'-v','error','-nostdin','-f','lavfi','-i','color=c=blue:s=160x90:r=15','-t','610','-c:v','libx264','-preset','ultrafast','-g','150','-keyint_min','150','-sc_threshold','0','-bf','0','-y',str(master)],check=True,capture_output=True,timeout=45)
            parts=_request(0,'video_split_local',root=str(root/'parts'),ffmpeg=ffmpeg,master=str(master),seconds=610)
            self.assertEqual([(p['start_seconds'],p['end_seconds'])for p in parts],[(0,300),(290,590),(580,610)])
            self.assertTrue(all(p['bytes']<=150_000_000 for p in parts))
            self.assertTrue(all((root/'parts'/'00000000000000000000000000000001'/p['name']).is_file()for p in parts))
