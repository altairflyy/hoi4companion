import copy,json,subprocess,unittest
from pathlib import Path
from hoi4companion.contracts import normalise_state


class SlotEvidenceTests(unittest.TestCase):
    def test_duplicate_invalid_or_unbounded_slots_are_unknown_with_runtime_parity(self):
        cases=[]
        for slots in ([{'slot':1,'technology_id':None}]*2,
                      [{'slot':True,'technology_id':None}],
                      [{'slot':-1,'technology_id':None}],
                      [{'slot':65,'technology_id':None}],
                      [{'technology_id':None}]*65):
            cases.append({'usable':True,'sample':{'country':'TST','research_slots':slots}})
            cases.append({'schema_version':1,'usable':True,'facts':{'research.slots':{'status':'observed','source':'fixture','value':slots}}})
        for value in cases:self.assertEqual(normalise_state(value)['facts']['research.slots']['status'],'unknown')
        script="const fs=require('fs'),c=require(process.argv[1]);process.stdout.write(JSON.stringify(JSON.parse(fs.readFileSync(0,'utf8')).map(c.state)));"
        result=subprocess.run(['node','-e',script,str(Path('src/hoi4companion/web/contracts.js').resolve())],input=json.dumps(cases),text=True,capture_output=True,check=True)
        self.assertEqual(json.loads(result.stdout),[normalise_state(value) for value in cases])

    def test_legacy_unindexed_and_zero_based_slots_remain_compatible(self):
        for slots in ([],[{'technology_id':None}],
                      [{'slot':0,'technology_id':None},{'slot':1,'technology_id':'radio'}],
                      [{'slot':i,'technology_id':None} for i in range(1,65)]):
            value=normalise_state({'usable':True,'sample':{'country':'TST','research_slots':slots}})
            self.assertEqual(value['facts']['research.slots']['value'],slots)

    def test_absent_or_malformed_breakthrough_section_does_not_crash_adapter(self):
        for points in (None,[],False,'unavailable'):
            raw={'usable':True,'sample':{'country':'TST','country_overview':{'country':'TST','projects':{'status':'partial','breakthroughs':points}}}}
            value=normalise_state(raw)
            self.assertFalse(any(key.startswith('projects.points.') for key in value['facts']))
