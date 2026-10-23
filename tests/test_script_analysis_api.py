import json
from pathlib import Path
from tempfile import TemporaryDirectory
from threading import RLock,Thread
from unittest.mock import patch
from urllib.request import urlopen
from urllib.error import HTTPError
from http.server import ThreadingHTTPServer
import unittest
from hoi4companion.api import Store,make_handler
from hoi4companion.script_analysis import RuleBook,RULE_ROOT


class ScriptAnalysisApiTests(unittest.TestCase):
    def test_static_apis_and_rules_do_not_need_a_live_reader(self):
        store=Store.__new__(Store);store.lock=RLock();store._analysis_lock=RLock()
        store._analysis_report=None;store._analysis_generation=1
        store.demo=True;store.labels={};store.reference_labels={}
        with ThreadingHTTPServer(('127.0.0.1',0),make_handler(store)) as server:
            thread=Thread(target=server.serve_forever,daemon=True);thread.start()
            base=f'http://127.0.0.1:{server.server_port}'
            try:
                with urlopen(base+'/api/script-analysis') as response:report=json.load(response)
                self.assertTrue(report['demo'])
                self.assertFalse(report['runtime_evaluated'])
                with urlopen(base+'/api/script-rules?kind=effect&name=set_capital') as response:rule=json.load(response)
                self.assertTrue(rule['variants'])
                with self.assertRaises(HTTPError):urlopen(base+'/api/script-rules?kind=invalid&name=a')
                with urlopen(base+'/analysis') as response:self.assertIn(b'Analisi script',response.read())
            finally:server.shutdown();thread.join(3)

    def test_analysis_is_cached_and_stale_generation_is_not_published(self):
        store=Store.__new__(Store);store.lock=RLock();store._analysis_lock=RLock()
        store._analysis_report=None;store._analysis_generation=1;store.demo=False
        store.game_root=Path('fixture');store.mod_roots=[];store.labels={};store.reference_labels={}
        with patch('hoi4companion.script_analysis.analyze_installation',return_value={'fixture':True}) as analyze:
            self.assertEqual(store.script_analysis(),{'fixture':True})
            store.script_analysis();self.assertEqual(analyze.call_count,1)
            store.script_analysis(refresh=True);self.assertEqual(analyze.call_count,2)
        def reload(*args):store._analysis_generation+=1;return {'stale':True}
        with patch('hoi4companion.script_analysis.analyze_installation',side_effect=reload):
            with self.assertRaises(ValueError):store.script_analysis(refresh=True)
        self.assertNotEqual(store._analysis_report,{'stale':True})

    def test_tampered_rules_are_rejected(self):
        import shutil
        with TemporaryDirectory() as directory:
            root=Path(directory)/'rules';shutil.copytree(RULE_ROOT,root)
            (root/'triggers.cwt').write_text('tampered',encoding='utf-8')
            with self.assertRaisesRegex(ValueError,'modificate'):RuleBook(root)
