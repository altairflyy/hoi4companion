import io
import json
import os
from pathlib import Path
from tempfile import TemporaryDirectory
import threading
import time
import unittest
from unittest.mock import patch


class FakeFeed:
    def __init__(self):
        self.lock=threading.Lock();self.closed=threading.Event();self.guide_id='test'
        self.version=7;self.value=[{'key':'step','panel':'research','identity':'basic_tools',
            'labels':['Basic Tools'],'title':'Tools','status':'ready','folder':'industry'}]
        self.updated=time.monotonic();self.error='';self.guide_title='Fixture guide'
    def snapshot(self):return self.version,list(self.value),self.updated,self.error,self.guide_title
    def get(self,path):return {'guides':[{'id':'test','title':'Fixture guide'}]}
    def get_bytes(self,path):raise AssertionError('No fixture sprites requested')
    def select(self,guide):self.guide_id=guide;self.version+=1;self.value=[];self.updated=0


class FakeSprites:
    def __init__(self):self.calls=0
    def match(self,image,proposals,ocr):self.calls+=1;return []
    def navigation(self,image,proposals,matches):return []


def ocr():
    def line(text,x,y,width=90):
        return {'text':text,'words':[{'text':text,'x':x,'y':y,'width':width,'height':14}]}
    return {'width':400,'height':300,'lines':[line('Research',10,90),line('Basic Tools',120,150)]}


class OverlayBridgeTests(unittest.TestCase):
    def create(self):
        from hoi4companion.overlay_bridge import Bridge
        feed,sprites=FakeFeed(),FakeSprites()
        return Bridge(feed,sprites=sprites),feed,sprites

    def frame(self,bridge,token=1):
        request={'schema_version':1,'kind':'frame','token':token,'version':7,
            'view':[99,42,0,0,400,300],'width':400,'height':300,
            'raw_len':400*300*4,'ocr':ocr()}
        return bridge.handle(request,b'\0'*(400*300*4))

    def test_snapshot_and_match_preserve_runtime_targets_and_exact_ocr(self):
        bridge,feed,sprites=self.create()
        snapshot=bridge.handle({'schema_version':1,'kind':'snapshot','token':1})
        self.assertTrue(snapshot['usable'])
        self.assertEqual(snapshot['proposed'],feed.value)
        self.assertEqual(snapshot['guides'][0]['id'],'test')
        result=self.frame(bridge)
        self.assertTrue(result['usable'])
        self.assertEqual(result['matches'][0]['identity'],'basic_tools')
        self.assertEqual(result['matches'][0]['basis'],'exact_unique_ocr')
        self.assertEqual(result['view'],[99,42,0,0,400,300])
        self.assertEqual(sprites.calls,1)
        cached=self.frame(bridge,2)
        self.assertTrue(cached['cache_hit'])
        self.assertEqual(sprites.calls,1)

    def test_idle_explanation_never_hides_a_stale_or_invalid_live_state(self):
        bridge,feed,_=self.create();feed.value=[]
        feed.explanation=lambda version:{'code':'slots_occupied','message':'Slot occupati'}
        self.assertEqual(bridge.snapshot()['idle_explanation']['code'],'slots_occupied')
        feed.updated=0;self.assertIsNone(bridge.snapshot()['idle_explanation'])
        feed.updated=time.monotonic();feed.error='Lettura non valida'
        self.assertIsNone(bridge.snapshot()['idle_explanation'])

    def test_stale_revision_invalid_feed_and_mid_recognition_change_fail_closed(self):
        bridge,feed,sprites=self.create();feed.version=8
        self.assertFalse(self.frame(bridge)['usable'])
        self.assertEqual(sprites.calls,0)
        feed.version=7
        def changed(*args):feed.version=8;return []
        sprites.match=changed
        result=self.frame(bridge)
        self.assertFalse(result['usable']);self.assertEqual(result['matches'],[])
        feed.updated=0
        self.assertFalse(bridge.handle({'schema_version':1,'kind':'snapshot'})['usable'])

    def test_binary_stream_framing_does_not_encode_images_or_desynchronize(self):
        from hoi4companion.overlay_bridge import serve
        bridge,feed,sprites=self.create()
        frame={'schema_version':1,'kind':'frame','token':3,'version':7,
            'view':[99,42,0,0,400,300],'width':400,'height':300,
            'raw_len':400*300*4,'ocr':ocr()}
        close={'schema_version':1,'kind':'close','token':4}
        stream=io.BytesIO(json.dumps(frame).encode()+b'\n'+b'\0'*(400*300*4)+json.dumps(close).encode()+b'\n')
        output=io.BytesIO();serve(bridge,stream,output)
        replies=[json.loads(line) for line in output.getvalue().splitlines()]
        self.assertEqual([r['token'] for r in replies],[3,4])
        self.assertTrue(replies[0]['usable']);self.assertTrue(feed.closed.is_set())

    def test_malformed_dimensions_truncated_frames_and_unknown_protocol_are_errors(self):
        from hoi4companion.overlay_bridge import serve,MAX_FRAME_BYTES
        bridge,feed,sprites=self.create()
        for request in ({'schema_version':2,'kind':'snapshot'},
                        {'schema_version':1,'kind':'frame','raw_len':MAX_FRAME_BYTES+1}):
            output=io.BytesIO();serve(bridge,io.BytesIO(json.dumps(request).encode()+b'\n'),output)
            self.assertFalse(json.loads(output.getvalue())['ok'])
        bridge,feed,sprites=self.create();output=io.BytesIO()
        serve(bridge,io.BytesIO(b'{"schema_version":1,"kind":"frame","raw_len":12}\nshort'),output)
        self.assertFalse(json.loads(output.getvalue())['ok'])

    def test_guide_selection_invalidates_old_frames(self):
        bridge,feed,sprites=self.create()
        result=bridge.handle({'schema_version':1,'kind':'select','guide_id':'test','token':4})
        self.assertFalse(result['usable']);self.assertEqual(feed.version,8)
        self.assertFalse(self.frame(bridge)['usable'])
        with self.assertRaises(ValueError):bridge.handle({'schema_version':1,'kind':'select','guide_id':'unknown'})

    def test_server_outage_keeps_controls_alive_and_hides_until_recovery(self):
        bridge,feed,_=self.create()
        original=feed.get
        feed.get=lambda path: (_ for _ in ()).throw(ConnectionError('offline'))
        result=bridge.handle({'schema_version':1,'kind':'snapshot','token':1})
        self.assertTrue(result['ok'])
        self.assertFalse(result['usable'])
        self.assertEqual(result['proposed'],[])
        self.assertIn('Server non disponibile',result['error'])
        feed.get=original
        self.assertTrue(bridge.handle({'schema_version':1,'kind':'snapshot','token':2})['usable'])


class NativeOverlayLauncherTests(unittest.TestCase):
    def test_hidden_python_launcher_uses_console_interpreter_for_binary_protocol(self):
        from hoi4companion.overlay_native import _console_python
        with TemporaryDirectory() as temp:
            console=Path(temp)/'python.exe';console.write_bytes(b'fixture')
            with patch('hoi4companion.overlay_native.sys.executable',str(Path(temp)/'Pythonw.exe')):
                self.assertEqual(_console_python(),str(console))

    def test_missing_native_backend_is_explicit_and_never_runs_legacy_overlay(self):
        from hoi4companion.overlay_native import native_executable
        with patch.dict(os.environ,{'HOI4_OVERLAY_NATIVE':'missing-overlay.exe'}):
            with self.assertRaisesRegex(RuntimeError,'Overlay Rust'):native_executable()

    def test_command_preserves_options_and_passes_python_without_shell_interpolation(self):
        from hoi4companion.overlay_native import command
        with TemporaryDirectory() as temp:
            executable=Path(temp)/'overlay with spaces.exe';executable.write_bytes(b'fixture')
            with patch.dict(os.environ,{'HOI4_OVERLAY_NATIVE':str(executable)}):
                args=command(port=8772,guide='guide with spaces',start=True,proof=Path(temp)/'proof.png')
            self.assertEqual(args[0],str(executable.resolve()))
            self.assertIn('--bridge-python',args)
            self.assertIn('guide with spaces',args)
            self.assertIn('--start',args)
            self.assertIn('--proof',args)


if __name__=='__main__':unittest.main()
