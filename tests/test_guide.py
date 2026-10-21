from pathlib import Path
from types import SimpleNamespace
import unittest
import json
import threading
from threading import RLock
from http.server import ThreadingHTTPServer
from urllib.request import urlopen, Request
from urllib.error import HTTPError

from hoi4companion.guide import load_guide, enrich_guide


class GuideTest(unittest.TestCase):
    def test_check_api_is_shared_and_preserves_same_origin_validation(self):
        from hoi4companion.api import Store,make_handler
        store=Store.__new__(Store)
        store.lock=RLock();store.guide_checks={};store.guide_revision=0
        with ThreadingHTTPServer(('127.0.0.1',0),make_handler(store)) as server:
            thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
            url=f'http://127.0.0.1:{server.server_port}/api/guide/checks'
            try:
                request=Request(url,data=b'{"key":"f12","checked":true}',headers={'Content-Type':'application/json'})
                with urlopen(request) as response:self.assertEqual(json.load(response)['checks'],{'f12':True})
                with urlopen(url) as response:self.assertEqual(json.load(response),{'checks':{'f12':True},'revision':1})
                request=Request(url,data=b'{"reset":true}',headers={'Content-Type':'application/json','Origin':'https://example.invalid'})
                with self.assertRaises(HTTPError) as error:urlopen(request)
                self.assertEqual(error.exception.code,403)
                request=Request(url,data=b'{"key":"f01","checked":true}',headers={'Content-Type':'application/json'})
                with self.assertRaises(HTTPError) as error:urlopen(request)
                self.assertEqual(error.exception.code,400)
            finally:
                server.shutdown();thread.join(timeout=2)

    def test_shared_checks_only_accept_declared_manual_keys(self):
        from hoi4companion.guide import update_checks
        guide=load_guide()
        result=update_checks(guide,{}, {'key':'f12','checked':True})
        self.assertEqual(result,{'f12':True})
        self.assertEqual(update_checks(guide,result,{'reset':True}),{})
        with self.assertRaises(ValueError):
            update_checks(guide,{}, {'key':'f01','checked':True})
        with self.assertRaises(ValueError):
            update_checks(guide,{}, {'key':'f12','checked':'yes'})

    def test_video_plan_preserves_all_source_rows_and_explicit_romanov_ids(self):
        guide = load_guide()
        self.assertEqual(guide['country'], 'POL')
        self.assertEqual(len(guide['tracks']['focus']), 34)
        self.assertEqual(len(guide['tracks']['research']), 48)
        focuses = guide['tracks']['focus']
        self.assertEqual(focuses[10]['target'], 'POL_demand_LIT_pavel')
        self.assertEqual(focuses[13]['target'], 'POL_demand_slovakia_pavel')
        self.assertEqual(focuses[14]['target'], 'POL_pan_slavism')
        self.assertEqual(focuses[11]['kind'], 'gate')
        self.assertEqual(guide['source']['video_id'], 'FBPdhlwqMwM')

    def test_enrichment_never_claims_unknown_targets_are_available(self):
        data = {'country':'POL','tree_id':'p','tracks':{'focus':[{'kind':'focus','target':'missing'}],
                'research':[{'kind':'technology','target':'tech'}, {'kind':'project','target':'sp1'}]}}
        result = enrich_guide(data, {}, {'tech':{'name':'Generic','folders':[{'id':'industry_folder','layout':{}}],
                             'start_year':1936,'unlocks':[]}}, {'POL_tech':'National'}, {'industry_folder':{'name':'Industria'}})
        self.assertFalse(result['tracks']['focus'][0]['catalog_match'])
        self.assertEqual(result['tracks']['research'][0]['name'],'National')
        self.assertTrue(result['tracks']['research'][0]['catalog_match'])
        self.assertEqual(result['tracks']['research'][1]['verification'],'manual')
        self.assertIn('missing',result['catalog_warnings'][0])


if __name__ == '__main__':
    unittest.main()
