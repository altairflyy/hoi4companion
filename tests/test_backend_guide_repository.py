"""Installed definitions and local corrections are separate from campaign runs."""
from pathlib import Path
from tempfile import TemporaryDirectory
import json
import unittest
from hoi4companion.backend_core import _request

class BackendGuideRepositoryTests(unittest.TestCase):
    def test_directory_order_and_local_corrections_do_not_rewrite_original(self):
        with TemporaryDirectory() as folder:
            root=Path(folder);bundled=root/'bundled';local=root/'local';changes=root/'changes'
            for path in (bundled,local,changes):path.mkdir()
            guide={'id':'z','title':'First','tracks':{'focus':[{'key':'f','action':{'kind':'select_focus','entity':{'kind':'focus','id':'wrong','scope':'tree'}}}]}}
            path=bundled/'one.json';path.write_text(json.dumps(guide));before=path.read_bytes()
            (local/'a.json').write_text(json.dumps({'id':'a','title':'Second','tracks':{}}))
            (changes/'z.json').write_text(json.dumps({'schema_version':1,'guide_id':'z','changes':[{'key':'f','original':{'kind':'focus','id':'wrong','scope':'tree'},'entity':{'kind':'focus','id':'correct','scope':'tree'}}]}))
            handle=_request(0,'app_open',guide_directories=[str(bundled),str(local)],association_directory=str(changes))
            self.addCleanup(lambda:_request(0,'app_release',handle=handle))
            self.assertEqual(_request(0,'app_active',handle=handle)['guide_id'],'z')
            loaded=_request(0,'app_guide',handle=handle,guide_id='z')
            self.assertEqual(loaded['tracks']['focus'][0]['target'],'correct')
            self.assertEqual(path.read_bytes(),before)
            (local/'duplicate.json').write_text(json.dumps(guide))
            with self.assertRaisesRegex(ValueError,'duplicat'):_request(0,'app_open',guide_directories=[str(bundled),str(local)])

    def test_stale_corrections_are_reported_and_unknown_schema_is_rejected(self):
        with TemporaryDirectory() as folder:
            root=Path(folder);(root/'guides').mkdir();(root/'corrections').mkdir()
            guide={'id':'g','tracks':{'research':[{'key':'r','action':{'kind':'research','entity':{'kind':'technology','id':'current'}}}]}}
            (root/'guides/g.json').write_text(json.dumps(guide))
            changes={'schema_version':1,'guide_id':'g','changes':[{'key':'r','original':{'kind':'technology','id':'old'},'entity':{'kind':'technology','id':'replacement'}}]}
            path=root/'corrections/g.json';path.write_text(json.dumps(changes))
            result=_request(0,'guide_repository',directories=[str(root/'guides')],association_directory=str(root/'corrections'))
            self.assertEqual(result['guides'][0]['tracks']['research'][0]['target'],'current')
            self.assertTrue(result['guides'][0]['association_warnings'])
            changes['schema_version']=2;path.write_text(json.dumps(changes))
            with self.assertRaises(ValueError):_request(0,'guide_repository',directories=[str(root/'guides')],association_directory=str(root/'corrections'))
