import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from hoi4companion.guide_importer import preview
from hoi4companion.guide_runtime import evaluate
from hoi4companion.political_import import political_catalog
from hoi4companion.resolution import CatalogIndex
import test_informal_operations as operations
from test_production_remaining import CATALOG,line,state


class ApprovedInformalTests(unittest.TestCase):
    def report(self,text,catalog=CATALOG):return operations.InformalOperationsTests().report([text],catalog)

    def test_asap_without_colon_and_experience_currency_follow_unique_spirit_group(self):
        catalog=[{'kind':'idea','id':'political_loyalty_spirit','name':'Political Loyalty','aliases':[],'group':'academy_spirit'},
                 {'kind':'idea','id':'branch_independence_spirit','name':'Branch Independence Spirit','aliases':[],'group':'air_force_spirit'}]
        for text,key,currency in [('ASAP PL Spirit (20AE)','political_loyalty_spirit','army_experience'),
                                  ('Branch Independence Spirit (50AE)','branch_independence_spirit','air_experience')]:
            report,_=self.report(text,catalog);step=report['guide']['tracks']['operations'][0]
            self.assertEqual(step['action']['params']['idea_id'],key)
            self.assertEqual(step['action']['params']['cost_currency'],currency)
        report,_=self.report('Branch Independence Spirit (50NE)',catalog)
        self.assertEqual(report['guide']['tracks']['operations'][0]['tracking'],'reminder')

    def test_authoring_production_priority_is_per_line_and_rejects_contradictory_modes(self):
        from hoi4companion.guide_authoring import compile_document
        from test_guide_authoring import document
        value=compile_document(document({'type':'production','equipment':'Infantry Equipment','factories':{'mode':'total','value':10},'production_priority':'first','priority':5}))
        self.assertEqual(value['guide']['tracks']['operations'][0]['action']['params']['production_priority'],'first')
        self.assertEqual(value['guide']['tracks']['operations'][0]['priority'],5)
        for fields in ({'production_priority':'middle'},{'production_priority':'first','production_category':'naval'},
                       {'production_priority':'first','factories':{'mode':'add','value':1}}):
            with self.assertRaises(ValueError):compile_document(document({'type':'production','equipment':'Infantry Equipment','factories':{'mode':'total','value':10},**fields}))

    def test_production_grammar_and_deferred_shared_numbers(self):
        for text in ('Production: Sup to 1, Inf Equip rest','MIL: Sup 1, Inf ALL'):
            report,sources=self.report(text);row=report['guide']['tracks']['operations'][0]
            self.assertEqual(row['action']['params']['production_targets'][0]['requested_factories'],1)
            self.assertEqual(row['action']['params']['production_targets'][1]['quantity_mode'],'remaining')
            self.assertEqual(preview(report['guide'],sources,CATALOG)['guide'],report['guide'])
        for text in ('Production: Sup, Inf, 10','Production: Sup & Inf 5','Production: Art to 2 F1 2'):
            report,_=self.report(text)
            self.assertTrue(all(r.get('tracking')=='reminder' for r in report['guide']['tracks']['operations']))

    def test_two_top_priority_targets_allow_either_internal_order_but_no_intervening_line(self):
        report,sources=self.report('MIL: Sup to 1, Truck to 2 (both top PRIO), Inf ALL')
        guide=report['guide'];row=guide['tracks']['operations'][0]
        self.assertEqual([p.get('production_priority') for p in row['action']['params']['production_targets']],['first','first',None])
        states=[state([line('t','truck',2),line('s','sup',1),line('i','inf',20)]),
                state([line('x','other',1),line('s','sup',1),line('t','truck',2),line('i','inf',20)])]
        self.assertEqual(evaluate(guide,states[0])['steps'][0]['status'],'completed')
        self.assertNotEqual(evaluate(guide,states[1])['steps'][0]['status'],'completed')
        self.assertEqual(preview(guide,sources,CATALOG)['guide'],guide)
        module=(Path(__file__).parent.parent/'src/hoi4companion/web/guide-runtime.js').as_posix()
        script='const r=require(process.argv[1]);let t="";process.stdin.on("data",x=>t+=x);process.stdin.on("end",()=>{let p=JSON.parse(t);process.stdout.write(JSON.stringify(p.states.map(s=>r.evaluate(p.guide,s))));});'
        out=subprocess.run(['node','-e',script,module],input=json.dumps({'guide':guide,'states':states}),capture_output=True,text=True,check=True)
        self.assertEqual(json.loads(out.stdout),[evaluate(guide,s) for s in states])

    def test_unresolved_member_of_top_group_preserves_group_and_does_not_reorder_other_member(self):
        report,_=self.report('MIL: Unknown Model to 3, Sup to 1 (both top PRIO), Inf ALL')
        rows=report['guide']['tracks']['operations']
        targets=[p for row in rows for p in row['action'].get('params',{}).get('production_targets',[])]
        self.assertFalse(any(p.get('equipment_name')=='support equipment' for p in targets))
        residual=' '.join(str(r['action']['params'].get('instructions')) for r in rows if r.get('tracking')=='reminder')
        self.assertIn('Unknown Model',residual);self.assertIn('both top PRIO',residual)

    def test_lowest_priority_is_checked_and_explicit_completion_is_preserved(self):
        report,_=self.report('MIL: Inf 10 (lowest prio)')
        guide=report['guide']
        self.assertEqual(evaluate(guide,state([line('x','other',1),line('i','inf',10)]))['steps'][0]['status'],'completed')
        self.assertNotEqual(evaluate(guide,state([line('i','inf',10),line('x','other',1)]))['steps'][0]['status'],'completed')
        self.assertIsNone(evaluate(guide,{'usable':False})['steps'][0]['completion']['value'])

    def test_research_and_focus_timing_are_exact_predicates_and_explicit_when_wins(self):
        catalog=CATALOG+[{'kind':'technology','id':'engines_3','name':'Engines 3','aliases':[]},
                         {'kind':'focus','id':'defensive','name':'Defensive Stratagems','scope':'test','aliases':[]}]
        for timing,key,values in [('On Engines 3','research.completed',['engines_3']),
                                  ('When Picking “Defensive Stratagems”','focus.active',['defensive'])]:
            report,_=self.report(timing+':\nProduction: Sup to 1',catalog)
            guide=report['guide'];s=state([line('s','sup',0)])
            self.assertNotEqual(evaluate(guide,s)['steps'][0]['status'],'completed')
            s['facts'][key]={'status':'observed','source':'fixture','value':values}
            self.assertEqual(evaluate(guide,s)['steps'][0]['status'],'ready')
        def edit(g):g['tracks']['operations'][0]['when']={'op':'fact','key':'authored','test':'eq','value':True}
        report,_=operations.InformalOperationsTests().report(['On Engines 3: Production: Sup to 1'],catalog,edit)
        self.assertEqual(report['guide']['tracks']['operations'][0]['when']['key'],'authored')

    def test_national_character_file_scopes_shared_traits_and_names_remain_fallback(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);folder=root/'common/characters';folder.mkdir(parents=True)
            for country,person in [('TST','first'),('ALT','second')]:
                (folder/(country+'.txt')).write_text(f'characters={{ {person}={{ name={person} advisor={{ idea_token={person}_role slot=political_advisor traits={{ shared_trait }} }} }} }}')
            rows,_=political_catalog(root,[],{'shared_trait':'Statista impeccabile'},{'shared_trait':'Impeccable Statesman'})
            report,_=self.report('Impeccable Statesman (150PP)',rows)
            self.assertEqual(report['guide']['tracks']['operations'][0]['action']['params']['advisor_id'],'first_role')
            report,_=self.report('Statista impeccabile (150PP)',rows)
            self.assertEqual(report['guide']['tracks']['operations'][0]['action']['params']['advisor_id'],'first_role')
            rows.append({**copy.deepcopy(next(r for r in rows if r['countries']==['TST'])),'id':'third_role','character_id':'third','name':'Third Person'})
            report,_=self.report('Impeccable Statesman (150PP)',rows)
            self.assertEqual(report['guide']['tracks']['operations'][0]['tracking'],'reminder')
            report,_=self.report('Third Person (150PP)',rows)
            self.assertEqual(report['guide']['tracks']['operations'][0]['action']['params']['advisor_id'],'third_role')
