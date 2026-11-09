import copy
import json
from pathlib import Path
import subprocess
import unittest

import test_informal_operations as operations_tests
from hoi4companion.guide_importer import preview
from hoi4companion.guide_runtime import evaluate
from hoi4companion.screen_overlay import targets


class InformalFamiliesTests(unittest.TestCase):
    def setUp(self):self.helper=operations_tests.InformalOperationsTests()

    def catalog(self):
        return [
            {'kind':'advisor','id':'chief','character_id':'chief_person','name':'National General','aliases':['Army Offense'],'slot':'army_chief','countries':['TST']},
            {'kind':'advisor','id':'foreign_chief','character_id':'foreign_person','name':'Foreign General','aliases':['Army Offense'],'slot':'army_chief','countries':['ALT']},
            {'kind':'idea','id':'political_loyalty_spirit','name':'Political Loyalty','aliases':[],'group':'academy_spirit','countries':[]},
            {'kind':'decision','id':'restore','name':'Restore the Regional Union','group':'union_category','aliases':[],'countries':['TST']},
            {'kind':'decision','id':'foreign_restore','name':'Restore the Regional Union','group':'union_category','aliases':[],'countries':['ALT']},
            {'kind':'equipment','id':'support_equipment','name':'Support Equipment','family_id':'support_equipment','equipment_ids':['support_equipment_1'],'is_archetype':True,'aliases':[]},
            {'kind':'equipment','id':'infantry_equipment','name':'Infantry Equipment','family_id':'infantry_equipment','equipment_ids':['infantry_equipment_0','infantry_equipment_1'],'is_archetype':True,'aliases':[]},
        ]

    def state(self,**facts):return self.helper.state(**facts)

    def test_national_office_alias_and_editorial_condition_are_preserved(self):
        report,_=self.helper.report(['Army Offense (100PP) (before event)'],self.catalog())
        row=report['guide']['tracks']['operations'][0]
        self.assertEqual(row['action']['params']['advisor_id'],'chief')
        self.assertEqual(row['action']['params']['role'],'army_chief')
        self.assertEqual(row['political_power_cost'],100)
        self.assertIn('before event',row['action']['params']['timing'])
        self.assertEqual(evaluate(report['guide'],self.state())['steps'][0]['reason'],'condition_unknown')
        def other(guide):guide['applies_to']={'country':'ALT'}
        report,_=self.helper.report(['Army Offense (100PP)'],self.catalog(),edit=other)
        self.assertEqual(report['guide']['tracks']['operations'][0]['action']['params']['advisor_id'],'foreign_chief')

    def test_spirit_alias_cost_currency_and_membership_with_python_js_parity(self):
        report,sources=self.helper.report(['ASAP: PL Spirit (20AE)'],self.catalog())
        row=report['guide']['tracks']['operations'][0]
        self.assertEqual(row['action']['params']['idea_id'],'political_loyalty_spirit')
        self.assertEqual(row['action']['params']['cost_currency'],'army_experience')
        self.assertEqual(row['resource_costs'],{'country.experience.army':20})
        self.assertNotIn('political_power_cost',row)
        states=[self.state(**{'politics.ideas':[]}),self.state(**{'politics.ideas':[],'country.experience.army':10}),
                self.state(**{'politics.ideas':[],'country.experience.army':20}),
                self.state(**{'politics.ideas':['political_loyalty_spirit']})]
        results=[evaluate(report['guide'],s) for s in states]
        self.assertIn('resource_balance_unknown',results[0]['steps'][0]['action_readiness']['unknown'])
        self.assertNotIn('political_cost_unknown',results[0]['steps'][0]['action_readiness']['unknown'])
        self.assertEqual(results[1]['steps'][0]['status'],'waiting')
        self.assertEqual(results[2]['steps'][0]['status'],'ready')
        self.assertEqual(results[3]['steps'][0]['status'],'completed')
        for i in (0,1):self.assertEqual(targets(report['guide'],results[i],self.catalog(),{**states[i],'sample':{'country':'TST'}}),[])
        self.assertEqual(preview(report['guide'],sources,self.catalog())['guide'],report['guide'])
        module=(Path(__file__).parent.parent/'src/hoi4companion/web/guide-runtime.js').as_posix()
        script='const r=require(process.argv[1]);let s="";process.stdin.on("data",x=>s+=x);process.stdin.on("end",()=>{const p=JSON.parse(s);process.stdout.write(JSON.stringify(p.states.map(s=>r.evaluate(p.guide,s))));});'
        out=subprocess.run(['node','-e',script,module],input=json.dumps({'guide':report['guide'],'states':states}),text=True,capture_output=True,check=True)
        self.assertEqual(json.loads(out.stdout),results)

    def test_decision_is_an_action_but_listing_disappearance_and_manual_checks_are_not_execution(self):
        report,_=self.helper.report(['Restore the Regional Union'],self.catalog())
        row=report['guide']['tracks']['operations'][0]
        self.assertNotEqual(row.get('tracking'),'reminder')
        self.assertEqual(row['action']['params']['decision_id'],'restore')
        self.assertEqual(row['completion'],{'op':'fact','key':'decisions.executed','test':'contains','value':'restore'})
        progress={'schema_version':1,'guide_id':report['guide']['id'],'run_id':'fixture','checks':{row['key']:True}}
        for listed in ([],['restore']):
            runtime=evaluate(report['guide'],self.state(**{'decisions.listed':listed}),progress)
            self.assertIsNone(runtime['steps'][0]['completion']['value'])
        self.assertFalse(report['operations'][0]['automatic_completion'])
        self.assertEqual(evaluate(report['guide'],self.state(**{'decisions.executed':['restore']}))['steps'][0]['status'],'completed')

    def line(self,key,equipment,requested,assigned):
        return {'id':key,'equipment':equipment,'name':equipment,'category':'military','requested_factories':requested,
                'assigned_factories':assigned,'active_factories':assigned,'damaged_factories':0}

    def test_numeric_production_layout_uses_requested_not_only_effective_factories(self):
        report,sources=self.helper.report(['Production: Sup:1, Inf:10'],self.catalog())
        row=report['guide']['tracks']['operations'][0]
        self.assertEqual([r['requested_factories'] for r in row['action']['params']['production_targets']],[1,10])
        self.assertEqual(row['action']['params']['category'],'production')
        partial=self.state(**{'production.lines':[self.line('s','support_equipment_1',1,1),self.line('i','infantry_equipment_1',9,1)]})
        complete=self.state(**{'production.lines':[self.line('s','support_equipment_1',1,0),self.line('i','infantry_equipment_1',10,1)]})
        self.assertEqual(evaluate(report['guide'],partial)['steps'][0]['status'],'active')
        self.assertEqual(evaluate(report['guide'],complete)['steps'][0]['status'],'completed')
        self.assertIsNone(evaluate(report['guide'],self.state())['steps'][0]['completion']['value'])
        self.assertEqual(preview(report['guide'],sources,self.catalog())['guide'],report['guide'])

    def test_unknown_quantities_equipment_and_conflicting_duplicates_are_not_invented(self):
        expanded,_=self.helper.report(['Production: Sup Equip:1, Inf Equip:10'],self.catalog())
        self.assertEqual([p['requested_factories'] for p in expanded['guide']['tracks']['operations'][0]['action']['params']['production_targets']],[1,10])
        report,_=self.helper.report(['MIL: Sup:1, Inf:ALL, Mystery:4 (MIO)'],self.catalog())
        rows=report['guide']['tracks']['operations']
        known=[r for r in rows if r['action']['params'].get('production_targets')]
        self.assertEqual(len(known),1)
        self.assertEqual(len(known[0]['action']['params']['production_targets']),1)
        rest=' '.join(str(r['action']['params']['instructions']) for r in rows if r.get('tracking')=='reminder')
        for label in ('ALL','Mystery:4','MIO'):self.assertIn(label,rest)
        duplicate,_=self.helper.report(['Production: Sup:1, Sup:2'],self.catalog())
        self.assertTrue(all(r.get('tracking')=='reminder' for r in duplicate['guide']['tracks']['operations']))
        entries=self.catalog()+[{'kind':'equipment','id':'infantry_equipment_1','name':'Rifle Model',
                                 'family_id':'infantry_equipment','equipment_ids':['infantry_equipment_1'],
                                 'is_archetype':False,'aliases':[]}]
        overlap,_=self.helper.report(['Production: Inf:10, Rifle Model:5'],entries)
        self.assertTrue(all(r.get('tracking')=='reminder' for r in overlap['guide']['tracks']['operations']))

    def test_cross_kind_or_same_country_ambiguity_is_not_arbitrarily_selected(self):
        entries=self.catalog()+[{'kind':'decision','id':'conflict','name':'Army Offense','group':'x','countries':['TST']}]
        report,_=self.helper.report(['Army Offense (100PP)'],entries)
        self.assertEqual(report['guide']['tracks']['operations'][0]['tracking'],'reminder')

    def test_experience_currency_cannot_erase_pp_requirements_on_unrelated_actions(self):
        from hoi4companion.guide import normalise_guide
        report,_=self.helper.report(['Army Offense (100PP)'],self.catalog())
        altered=copy.deepcopy(report['guide'])
        altered['tracks']['operations'][0]['action']['params']['cost_currency']='army_experience'
        with self.assertRaises(ValueError):normalise_guide(altered)

    def test_legacy_implicit_barriers_are_not_changed_by_new_promotions(self):
        def legacy(guide):guide['dependency_policy']='legacy'
        report,_=self.helper.report(['Production: Sup:1, Inf:10','Restore the Regional Union'],self.catalog(),edit=legacy)
        self.assertTrue(all(row.get('tracking')=='reminder' for row in report['guide']['tracks']['operations']))
