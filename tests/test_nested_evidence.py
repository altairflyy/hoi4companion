import copy,json,subprocess,unittest
from pathlib import Path
from hoi4companion.contracts import normalise_state,evaluate_condition


class NestedEvidenceTests(unittest.TestCase):
    def test_unavailable_child_never_inherits_observed_parent_status(self):
        cases=[]
        for status in ('unavailable','unknown','stale'):
            cases.append({'usable':True,'sample':{'country':'TST','country_overview':{'country':'TST',
                'politics':{'status':'partial','power':{'status':status,'available':120},
                            'advisors':{'status':status,'items':[]}},
                'manpower':{'status':'partial','available':0}}}})
        for raw in cases:
            state=normalise_state(raw)
            for key in ('country.politics.power.available','country.politics.advisors.items.count'):
                self.assertEqual(state['facts'][key]['status'],'unknown')
                self.assertIsNone(evaluate_condition({'op':'fact','key':key,'test':'gte','value':0},state))
            self.assertEqual(state['facts']['country.manpower.available']['value'],0)
        script="const fs=require('fs'),c=require(process.argv[1]);process.stdout.write(JSON.stringify(JSON.parse(fs.readFileSync(0,'utf8')).map(c.state)));"
        result=subprocess.run(['node','-e',script,str(Path('src/hoi4companion/web/contracts.js').resolve())],input=json.dumps(cases),text=True,capture_output=True,check=True)
        self.assertEqual(json.loads(result.stdout),[normalise_state(raw) for raw in cases])

    def test_known_child_does_not_revive_an_unavailable_parent(self):
        raw={'usable':True,'sample':{'country':'TST','country_overview':{'country':'TST',
            'politics':{'status':'unavailable','power':{'status':'observed','available':120}}}}}
        self.assertEqual(normalise_state(raw)['facts']['country.politics.power.available']['status'],'unknown')
