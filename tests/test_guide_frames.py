import copy
import json
from pathlib import Path
import subprocess
import unittest

from hoi4companion.guide_frames import frame_reference


URL='https://www.youtube.com/watch?v=abcdefghijk'
OTHER='https://www.youtube.com/watch?v=lmnopqrstuv'


def case():
    step={'key':'design','action':{'kind':'manual','params':{}},
          'evidence':[{'source_id':'video','basis':'explicit','timestamp_seconds':489}]}
    guide={'sources':[{'id':'video','type':'youtube','url':URL}],
           'source_inventory':{'schema_version':1,'items':[
               {'id':'frame','source_id':'video','basis':'visual','timestamp_seconds':511,'step_keys':['design']}]}}
    return guide,step


class FrameReferenceTests(unittest.TestCase):
    def test_linked_visual_inventory_beats_spoken_time_without_changing_input(self):
        guide,step=case();before=copy.deepcopy((guide,step))
        self.assertEqual(frame_reference(guide,step),{'url':URL,'seconds':511})
        self.assertEqual((guide,step),before)

    def test_explicit_frame_choice_and_direct_visual_evidence_keep_precedence(self):
        guide,step=case();step['action']['params']['source_seconds']=530
        self.assertEqual(frame_reference(guide,step),{'url':URL,'seconds':530})
        step['action']['params'].clear()
        step['evidence'].append({'source_id':'video','basis':'visual','timestamp_seconds':520})
        self.assertEqual(frame_reference(guide,step),{'url':URL,'seconds':520})

    def test_excluded_or_unrelated_inventory_never_supplies_a_step_frame(self):
        for patch in [{'step_keys':['different']},{'excluded_reason':'Unused'}]:
            guide,step=case();guide['source_inventory']['items'][0].update(patch)
            self.assertEqual(frame_reference(guide,step),{'url':URL,'seconds':489})

    def test_source_and_timestamp_stay_paired_and_distinct_videos_are_not_guessed(self):
        guide,step=case();guide['sources'].append({'id':'other','type':'youtube','url':OTHER})
        step['evidence'][0]['source_id']='other'
        self.assertEqual(frame_reference(guide,step),{'url':URL,'seconds':511})
        guide['source_inventory']['items'].append({'id':'another','source_id':'other','basis':'visual','timestamp_seconds':520,'step_keys':['design']})
        self.assertIsNone(frame_reference(guide,step))

    def test_zero_legacy_source_and_canonical_url_deduplication(self):
        guide,step=case();guide['sources'].append({'id':'alias','type':'youtube','url':'https://youtu.be/abcdefghijk?t=5'})
        step['action']['params']['source_seconds']=0
        self.assertEqual(frame_reference(guide,step),{'url':URL,'seconds':0})
        guide={'source':{'url':URL}};step={'key':'legacy','action':{'kind':'manual','params':{'source_seconds':12}}}
        self.assertEqual(frame_reference(guide,step),{'url':URL,'seconds':12})

    def test_invalid_explicit_time_and_unrelated_web_source_do_not_fall_back(self):
        for invalid in [True,-1,float('nan'),14400]:
            guide,step=case();step['action']['params']['source_seconds']=invalid
            self.assertIsNone(frame_reference(guide,step))
        guide,step=case();guide['source_inventory']['items']=[]
        guide['sources'].append({'id':'web','type':'web','url':'https://example.com/article'})
        step['evidence'][0]['source_id']='web'
        self.assertIsNone(frame_reference(guide,step))

    def test_missing_time_is_not_invented_from_notes(self):
        guide,step=case();guide.pop('source_inventory');step['evidence']=[]
        step['note']='The design might appear at 8:31.'
        self.assertIsNone(frame_reference(guide,step))

    def test_python_and_javascript_select_identical_references(self):
        cases=[]
        for change in ['normal','explicit','zero','legacy','unrelated','excluded','ambiguous','web','invalid','no-inventory','port','short-url']:
            guide,step=case()
            if change=='explicit':step['action']['params']['source_seconds']=530
            if change=='zero':step['action']['params']['source_seconds']=0
            if change=='legacy':guide={'source':{'url':URL}}
            if change=='unrelated':guide['source_inventory']['items'][0]['step_keys']=['other']
            if change=='excluded':guide['source_inventory']['items'][0]['excluded_reason']='Unused'
            if change=='ambiguous':
                guide['sources'].append({'id':'other','type':'youtube','url':OTHER})
                guide['source_inventory']['items'].append({'id':'another','source_id':'other','basis':'visual','timestamp_seconds':520,'step_keys':['design']})
            if change=='web':guide['sources'][0].update(type='web',url='https://example.com/article')
            if change=='invalid':step['action']['params']['source_seconds']=True
            if change=='no-inventory':guide['source_inventory']=None
            if change=='port':guide['sources'][0]['url']='https://www.youtube.com:443/watch?v=abcdefghijk'
            if change=='short-url':guide['sources'][0]['url']='https://youtu.be/abcdefghijk?t=100'
            cases.append({'guide':guide,'step':step})
        script="const cases=JSON.parse(require('fs').readFileSync(0,'utf8'));const p=require('./src/hoi4companion/web/guide-presentation.js');process.stdout.write(JSON.stringify(cases.map(x=>p.frameReference(x.guide,x.step))));"
        result=subprocess.run(['node','-e',script],cwd=Path(__file__).resolve().parents[1],input=json.dumps(cases),encoding='utf-8',capture_output=True,check=True)
        self.assertEqual(json.loads(result.stdout),[frame_reference(x['guide'],x['step']) for x in cases])

    def test_http_frame_endpoint_extracts_the_linked_visual_instead_of_the_audio_frame(self):
        from http.server import ThreadingHTTPServer
        from tempfile import TemporaryDirectory
        import threading
        from types import SimpleNamespace
        from unittest.mock import Mock
        from urllib.request import urlopen
        from urllib.error import HTTPError
        from hoi4companion.api import make_handler
        guide,step=case();guide['tracks']={'operations':[step]}
        with TemporaryDirectory() as root:
            image=Path(root)/'frame.jpg';image.write_bytes(b'visual-frame-fixture')
            frames=SimpleNamespace(frame=Mock(return_value=image))
            store=SimpleNamespace(walkthrough=lambda _:guide,guide_frames=frames)
            with ThreadingHTTPServer(('127.0.0.1',0),make_handler(store)) as server:
                worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start()
                try:
                    base=f'http://127.0.0.1:{server.server_port}/api/guide/frame?id=test&key='
                    with urlopen(base+'design') as response:
                        self.assertEqual(response.headers['Content-Type'],'image/jpeg')
                        self.assertEqual(response.read(),b'visual-frame-fixture')
                    frames.frame.assert_called_once_with(URL,511)
                    with self.assertRaises(HTTPError):urlopen(base+'absent')
                    self.assertEqual(frames.frame.call_count,1)
                finally:server.shutdown();worker.join(timeout=3)
