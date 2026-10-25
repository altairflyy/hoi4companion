"""The new projection accepts existing Python/JS runtime output equivalently."""
import copy
import json
from pathlib import Path
import shutil
import subprocess
import threading
from http.server import ThreadingHTTPServer
from types import SimpleNamespace
import unittest

from hoi4companion.api import make_handler
from hoi4companion.guide_runtime import evaluate

ROOT=Path(__file__).resolve().parents[1]


class OperationalSummaryTests(unittest.TestCase):
    def test_existing_python_and_js_runtime_evaluations_produce_the_same_summary(self):
        if not shutil.which('node'):self.skipTest('Node richiesto per la verifica incrociata')
        cases=[]
        for filename in ('production-progress.json','construction-progress.json','guide-runtime.json'):
            for case in json.loads((ROOT/'tests/fixtures'/filename).read_text(encoding='utf8')):
                if not isinstance(case,dict) or not all(k in case for k in ('guide','state')):continue
                record={k:copy.deepcopy(case.get(k)) for k in ('guide','state','progress')}
                record['name']=case.get('name',filename)
                record['python_runtime']=evaluate(record['guide'],record['state'],record['progress'])
                cases.append(record)
        self.assertGreater(len(cases),10)
        script="""const fs=require('node:fs'),assert=require('node:assert/strict');
const summary=require('./src/hoi4companion/web/operational-summary.js');
const runtime=require('./src/hoi4companion/web/guide-runtime.js');
const cases=JSON.parse(fs.readFileSync(0,'utf8'));
for(const c of cases){const before=JSON.stringify(c);
assert.deepEqual(summary.project(c.guide,c.python_runtime,c.state),
 summary.project(c.guide,runtime.evaluate(c.guide,c.state,c.progress),c.state),c.name);
assert.equal(JSON.stringify(c),before,c.name);}
process.stdout.write(JSON.stringify({cases:cases.length}));"""
        result=subprocess.run(['node','-e',script],cwd=ROOT,input=json.dumps(cases),capture_output=True,text=True,timeout=30)
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertEqual(json.loads(result.stdout)['cases'],len(cases))

    def test_summary_assets_are_served_by_the_existing_http_handler(self):
        from urllib.request import urlopen
        with ThreadingHTTPServer(('127.0.0.1',0),make_handler(SimpleNamespace())) as server:
            thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
            try:
                for path,expected in (('/operational-summary.js',b'HoiOperationalSummary'),('/walkthrough',b'operationalSummaryTitle')):
                    with urlopen(f'http://127.0.0.1:{server.server_port}'+path) as response:
                        self.assertEqual(response.status,200);self.assertIn(expected,response.read())
            finally:server.shutdown();thread.join(timeout=3)
