from contextlib import contextmanager
from http.server import ThreadingHTTPServer
import json
from pathlib import Path
import tempfile
from threading import RLock,Thread
import unittest
from unittest.mock import patch
from urllib.request import urlopen,Request
from urllib.error import HTTPError

from hoi4companion.api import Store,make_handler
from hoi4companion.guide_progress import RunStore
from hoi4companion.guide import normalise_guide
from hoi4companion.campaign import SOURCE
from test_campaign_progress import A,B


class CampaignApiTests(unittest.TestCase):
    def test_startup_populates_legacy_and_sse_checks_from_disk_without_a_state_change(self):
        with tempfile.TemporaryDirectory() as folder:
            settings=Path(folder)/'settings.json';runs=RunStore(path=settings.with_name('guide-progress.sqlite3'))
            runs.ensure('g');runs.bind_campaign(A,'TST','session',100);runs.update('g',{'done':True})
            with patch('hoi4companion.api.SETTINGS',settings),patch.object(Store,'refresh'),patch('hoi4companion.api.default_guide_id',return_value='g'):
                store=Store()
                try:
                    self.assertEqual(store.guide_checks,{'done':True})
                    self.assertEqual(store.guide_states['g'],{'done':True})
                    self.assertEqual(store._storage_status()['status'],'persistent')
                finally:store.close()

    def test_corrupt_archive_is_visible_and_writes_are_rejected(self):
        guide=normalise_guide({'id':'g','tracks':{'operations':[{'key':'done','action':{'kind':'manual'}}]}})
        with tempfile.TemporaryDirectory() as folder:
            settings=Path(folder)/'settings.json';database=settings.with_name('guide-progress.sqlite3');database.write_bytes(b'corrupt')
            with patch('hoi4companion.api.SETTINGS',settings),patch.object(Store,'refresh'),patch('hoi4companion.api.default_guide_id',return_value='g'),patch('hoi4companion.api.load_guide',return_value=guide):
                store=Store()
                try:
                    self.assertEqual(store._storage_status()['status'],'unavailable')
                    run=store.walkthrough_progress()
                    with self.assertRaises(ValueError):store.walkthrough_progress({'run_id':run['run_id'],'checks':{'done':True}})
                    self.assertEqual(database.read_bytes(),b'corrupt')
                finally:store.close()

    def test_http_restart_restores_campaign_and_rejects_stale_tabs(self):
        guide=normalise_guide({'id':'g','tracks':{'operations':[{'key':'done','action':{'kind':'manual'}}]}})
        state={'usable':True,'sample':{'country':'TST','session_id':'one','native_tick':100,
                'campaign_identity':{'status':'observed','id':A,'source':SOURCE}}}
        with tempfile.TemporaryDirectory() as folder,patch('hoi4companion.api.load_guide',return_value=guide):
            path=Path(folder)/'progress.sqlite3'
            def store():
                result=Store.__new__(Store);result.lock=RLock();result.default_guide_id='g'
                result.guide_runs=RunStore(path=path);result.guide_checks={};result.guide_states={};result.guide_revision=0
                result.live_cache=state
                return result
            @contextmanager
            def serve(current):
                with ThreadingHTTPServer(('127.0.0.1',0),make_handler(current)) as http:
                    thread=Thread(target=http.serve_forever,daemon=True);thread.start()
                    def call(body=None):
                        request=Request(f'http://127.0.0.1:{http.server_port}/api/guide/progress',
                            data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
                        with urlopen(request,timeout=2) as response:return json.load(response)
                    try:yield call
                    finally:http.shutdown();thread.join(timeout=2)
            with serve(store()) as call:
                first=call();self.assertEqual(first['storage']['status'],'persistent')
                guard={'server_id':first['server_id'],'expected_run_id':first['run_id'],'expected_revision':first['run_revision']}
                saved=call({'run_id':first['run_id'],'checks':{'done':True},**guard})
                with self.assertRaises(HTTPError):call({'run_id':first['run_id'],'checks':{'done':False},**guard})
                self.assertEqual(call()['checks'],{'done':True})
                identity=state['sample'].pop('campaign_identity')
                with self.assertRaises(HTTPError):call({'run_id':first['run_id'],'checks':{'done':False}})
                state['sample']['campaign_identity']=identity
                self.assertEqual(call()['checks'],{'done':True})
            state['sample']['session_id']='two'
            with serve(store()) as call:
                restored=call()
                self.assertEqual(restored['run_id'],first['run_id']);self.assertEqual(restored['checks'],{'done':True})
                self.assertEqual(restored['session_id'],'two')
                self.assertNotEqual(restored['server_id'],first['server_id'])
                with self.assertRaises(HTTPError):call({'run_id':restored['run_id'],'checks':{},'server_id':first['server_id']})
                state['sample']['campaign_identity']['id']=B
                other=call();self.assertNotEqual(other['run_id'],restored['run_id']);self.assertEqual(other['checks'],{})
                state['sample']['campaign_identity']['id']=A
                self.assertEqual(call()['checks'],{'done':True})


if __name__=='__main__':unittest.main()
