import copy,json,subprocess,unittest
from pathlib import Path
from hoi4companion.production_progress import production_context
from test_production_remaining import state,line


def fixture():
    targets=[]
    for key in ('sup','inf'):
        p={'category':'production','production_priority':'first','requested_factories':1,
           'production_selector':{'category':'military','equipment_ids':[key],'variant_names':[]}}
        if key=='sup':p['design_reference']={'modules':{'gun':'cannon'},'upgrades':{}}
        targets.append(p)
    return {'tracks':{'operations':[{'key':'layout','action':{'kind':'manual','params':{'production_targets':targets}}}]}}


class ProductionPriorityEvidenceTests(unittest.TestCase):
    def test_unknown_peer_is_not_a_proven_wrong_priority_and_unrelated_rows_still_block(self):
        guide=fixture();unknown={**line('s','sup',1),'design':{'status':'unavailable'}}
        cases=[(state([unknown,line('i','inf',1)]),None),
               (state([line('i','inf',1),unknown]),True),
               (state([unknown,line('t','truck',1),line('i','inf',1)]),False),
               (state([{**unknown,'design':{'status':'observed','modules':{'gun':'wrong'},'upgrades':{}}},line('i','inf',1)]),False),
               (state([{**unknown,'design':{'status':'observed','modules':{'gun':'cannon'},'upgrades':{}}},line('i','inf',1)]),True)]
        key='guide.production.layout.target-2.priority_satisfied'
        for value,expected in cases:
            context,_=production_context(guide,value,{})
            self.assertEqual(context['facts'][key]['value'],expected)
        script="const fs=require('fs'),p=require(process.argv[1]),x=JSON.parse(fs.readFileSync(0,'utf8'));process.stdout.write(JSON.stringify(x.states.map(s=>p.context(x.guide,s,{}).state.facts[x.key].value)));"
        run=subprocess.run(['node','-e',script,str(Path('src/hoi4companion/web/production-progress.js').resolve())],input=json.dumps({'guide':guide,'states':[s for s,_ in cases],'key':key}),text=True,capture_output=True,check=True)
        self.assertEqual(json.loads(run.stdout),[v for _,v in cases])
