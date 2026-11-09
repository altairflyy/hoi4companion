import json
from http.server import ThreadingHTTPServer
from threading import Thread,RLock
import unittest
from unittest.mock import patch
from urllib.request import urlopen

from hoi4companion.api import Store,make_handler
from hoi4companion.guide import normalise_guide


class GuideProjectionApiTests(unittest.TestCase):
    def test_projection_is_coherent_and_preview_does_not_poll_the_native_reader(self):
        store=Store.__new__(Store);store.lock=RLock();store.default_guide_id='g';store.guide_states={};store.guide_checks={};store.guide_revision=0
        store.trees={};store.labels={};store.technologies={'technologies':{},'folders':{}};store.settings={'country':'TST'};store.demo=True
        calls=[]
        live={'schema_version':1,'usable':True,'facts':{'focus.active':{'status':'observed','source':'fixture','value':[]},'focus.completed':{'status':'observed','source':'fixture','value':['A']}}}
        store.live_state=lambda:(calls.append('read') or live)
        guide=normalise_guide({'id':'g','tracks':{'focus':[{'key':'a','kind':'focus','target':'A'}]}})
        with patch('hoi4companion.api.load_guide',return_value=guide):
            with ThreadingHTTPServer(('127.0.0.1',0),make_handler(store)) as server:
                thread=Thread(target=server.serve_forever,daemon=True);thread.start()
                try:
                    base=f'http://127.0.0.1:{server.server_port}'
                    with urlopen(base+'/api/guide/projection?id=g&live=0',timeout=3) as response:preview=json.load(response)
                    self.assertEqual(calls,[]);self.assertEqual(preview['evaluation']['mode'],'preview')
                    with urlopen(base+'/api/guide/projection?id=g',timeout=3) as response:packet=json.load(response)
                    self.assertEqual(calls,['read']);self.assertEqual(packet['evaluation']['steps'][0]['status'],'completed')
                    self.assertEqual(packet['progress']['run_id'],packet['evaluation']['run_id'])
                    self.assertEqual(packet['state'],live);self.assertEqual(packet['guide_id'],'g')
                    self.assertEqual(packet['engine'],'rust-guide-core');self.assertEqual(len(packet['definition_revision']),64)
                finally:server.shutdown();thread.join(timeout=2)
