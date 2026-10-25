import json
from pathlib import Path
import tempfile
import threading
from http.server import ThreadingHTTPServer
import unittest
from unittest.mock import patch
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from hoi4companion.api import Store, make_handler
from hoi4companion.stream import state_signature


class ActiveGuideTests(unittest.TestCase):
    def test_selection_is_shared_validated_persisted_and_does_not_change_progress(self):
        with tempfile.TemporaryDirectory() as tmp:
            settings=Path(tmp)/'settings.json';settings.write_text('{}')
            store=Store.__new__(Store);store.lock=threading.RLock();store.active_guide_id='a'
            store.guide_checks={'keep':True};store.guide_revision=12
            def load(key):
                if key not in ('a','b'):raise ValueError('Missing guide')
                return {'id':key,'title':'Guide '+key}
            with patch('hoi4companion.api.SETTINGS',settings),patch('hoi4companion.api.load_guide',side_effect=load):
                self.assertEqual(store.active_guide()['guide_id'],'a')
                selected=store.active_guide({'guide_id':'b'})
                self.assertEqual(selected['revision'],1)
                self.assertEqual(store.active_guide({'guide_id':'b'})['revision'],1)
                self.assertEqual(json.loads(settings.with_name('active-guide.json').read_text()),{'guide_id':'b'})
                for bad in ({'guide_id':'missing'},{'guide_id':None},{'guide_id':'b','other':1},[]):
                    with self.assertRaises(ValueError):store.active_guide(bad)
                self.assertEqual(store.active_guide()['guide_id'],'b')
            self.assertEqual(store.guide_checks,{'keep':True});self.assertEqual(store.guide_revision,12)
            self.assertEqual(settings.read_text(),'{}')

    def test_http_selection_uses_existing_origin_guard_and_change_event_signature(self):
        class Selection:
            guide_id='a'
            def active_guide(self,update=None):
                if update:self.guide_id=update['guide_id']
                return {'guide_id':self.guide_id,'revision':1}
        store=Selection()
        with ThreadingHTTPServer(('127.0.0.1',0),make_handler(store)) as server:
            worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start()
            base=f'http://127.0.0.1:{server.server_port}/api/guide/active'
            try:
                with urlopen(base) as r:self.assertEqual(json.load(r)['guide_id'],'a')
                request=Request(base,data=b'{"guide_id":"b"}',headers={'Content-Type':'application/json'})
                with urlopen(request) as r:self.assertEqual(json.load(r)['guide_id'],'b')
                rejected=Request(base,data=b'{"guide_id":"a"}',headers={'Content-Type':'application/json','Origin':'https://other.invalid'})
                with self.assertRaises(HTTPError) as error:urlopen(rejected)
                self.assertEqual(error.exception.code,403)
                self.assertEqual(store.guide_id,'b')
            finally:server.shutdown();worker.join(timeout=3)
        self.assertNotEqual(state_signature({'usable':True,'active_guide_id':'a','active_guide_revision':0}),
                            state_signature({'usable':True,'active_guide_id':'b','active_guide_revision':1}))
