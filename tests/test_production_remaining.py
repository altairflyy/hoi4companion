import copy
import json
from pathlib import Path
import subprocess
import unittest

from hoi4companion.guide_authoring import compile_document
from hoi4companion.guide_importer import Importer, preview
from hoi4companion.guide_runtime import evaluate
from test_guide_authoring import document as authoring
from test_operation_import import document
import test_informal_operations as informal


CATALOG=[{'kind':'equipment','id':key,'name':name,'aliases':[],'equipment_ids':[key]}
         for key,name in [('sup','Support Equipment'),('inf','Infantry Equipment'),('truck','Truck')]]


def line(key,equipment,requested,assigned=0,category='military'):
    return {'id':key,'equipment':equipment,'name':equipment,'category':category,
            'requested_factories':requested,'assigned_factories':assigned,
            'active_factories':assigned,'damaged_factories':0}


def state(lines,total=20):
    facts={'country':'TST','production.lines':lines}
    if total is not None:facts['country.production.effective_factories.military']=total
    return {'schema_version':1,'usable':True,'facts':{
        k:{'status':'observed','source':'fixture','value':v} for k,v in facts.items()}}


def guide():
    g,s=document({'category':'production','production_targets':[
        {'equipment_id':'sup','requested_factories':1},
        {'equipment_id':'inf','quantity_mode':'remaining','production_category':'military'}]})
    return preview(g,s,CATALOG)['guide']


class ProductionRemainingTests(unittest.TestCase):
    def test_importer_preserves_numeric_counts_and_binds_only_final_military_all(self):
        report,sources=informal.InformalOperationsTests().report(['MIL: Sup:1, Inf:ALL'],CATALOG)
        row=report['guide']['tracks']['operations'][0]
        self.assertEqual(row['action']['params']['production_targets'][0]['requested_factories'],1)
        target=row['action']['params']['production_targets'][1]
        self.assertEqual(target['quantity_mode'],'remaining')
        self.assertEqual(target['production_selector']['category'],'military')
        self.assertEqual(report['operations'][0]['monitoring'],'automatic')
        self.assertEqual(preview(report['guide'],sources,CATALOG)['guide'],report['guide'])
        self.assertEqual(row['completion']['args'][1]['key'],'guide.production.'+row['key']+'.target-2.remaining_allocated')
        annotated,_=informal.InformalOperationsTests().report(['MIL: Sup:1, Inf:ALL (add MIOs)'],CATALOG)
        self.assertTrue(any(r['action'].get('params',{}).get('production_targets',[])[-1].get('quantity_mode')=='remaining'
            for r in annotated['guide']['tracks']['operations'] if r['action'].get('params',{}).get('production_targets')))
        self.assertTrue(any(r.get('tracking')=='reminder' and 'MIOs' in str(r) for r in annotated['guide']['tracks']['operations']))
        for text in ('MIL: Inf:ALL, Sup:1','MIL: Sup:ALL, Inf:ALL','NAV: Inf:ALL'):
            invalid,_=informal.InformalOperationsTests().report([text],CATALOG)
            self.assertFalse(any(p.get('quantity_mode')=='remaining'
                for r in invalid['guide']['tracks']['operations']
                for p in r['action'].get('params',{}).get('production_targets',[])),text)

    def test_all_subtracts_other_requested_lines_and_reacts_to_new_capacity(self):
        g=guide()
        lines=[line('1','sup',1,1),line('2','truck',3,0),line('3','inf',16,2),line('4','ship',100,0,'naval')]
        for total,requested,expected,target in [(20,16,'completed',16),(21,16,'active',17),(21,150,'completed',17),(3,150,'completed',0)]:
            lines[2]['requested_factories']=requested
            row=evaluate(g,state(lines,total))['steps'][0]
            self.assertEqual(row['status'],expected)
            self.assertEqual(row['production']['layout'][1]['target'],target)
        lines[0]['requested_factories']=10
        row=evaluate(g,state(lines,3))['steps'][0]
        self.assertNotEqual(row['status'],'completed')  # Sup:1 stays exactly one.

    def test_absence_zero_ambiguous_or_wrong_order_never_proves_completion(self):
        g=guide()
        cases=[([line('1','sup',1)],0),
               ([line('1','sup',1),line('2','inf',0)],1),
               ([line('1','sup',1),line('2','inf',150),line('3','inf',1)],20),
               ([line('1','inf',150),line('2','sup',1)],20)]
        for lines,total in cases:
            self.assertNotEqual(evaluate(g,state(lines,total))['steps'][0]['status'],'completed')

    def test_missing_capacity_invalid_capacity_stale_and_foreign_state_are_not_completion(self):
        g=guide();lines=[line('1','sup',1),line('2','inf',150)]
        for total in (None,-1,True,'20'):
            self.assertIsNone(evaluate(g,state(lines,total))['steps'][0]['completion']['value'])
        stale=state(lines);stale['usable']=False
        self.assertIsNone(evaluate(g,stale)['steps'][0]['completion']['value'])
        foreign=state(lines);foreign['facts']['country']['value']='ALT'
        self.assertEqual(evaluate(g,foreign)['mode'],'wrong_country')
        self.assertNotEqual(evaluate(g,foreign)['steps'][0]['status'],'completed')

    def test_authoring_remaining_has_no_numeric_value_and_requires_last_military_product(self):
        raw=authoring({'type':'production','lines':[
            {'equipment':'Support Equipment','factories':{'value':1}},
            {'equipment':'Infantry Equipment','factories':{'mode':'remaining'}}]})
        compiled=Importer(lambda:CATALOG).document(raw)
        report=preview(compiled['guide'],compiled['sources'],CATALOG)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual(report['operations'][0]['monitoring'],'automatic')
        for bad in ({'mode':'remaining','value':10},{'mode':'remaining','basis':'assigned'}):
            invalid=copy.deepcopy(raw);invalid['guide']['steps'][0]['lines'][-1]['factories']=bad
            with self.assertRaises(ValueError):compile_document(invalid)
        bad=copy.deepcopy(raw);bad['guide']['steps'][0]['lines'].reverse()
        with self.assertRaises(ValueError):compile_document(bad)
        bad=copy.deepcopy(raw);bad['guide']['steps'][0]['lines'][-1]['production_category']='naval'
        with self.assertRaises(ValueError):compile_document(bad)

    def test_python_javascript_runtime_parity(self):
        g=guide()
        lines=[line('1','sup',1,1),line('2','truck',3),line('3','inf',16,2)]
        states=[state(lines,total) for total in (20,21,None,-1)]
        states += [state([line('1','sup',1)],0),state(list(reversed(lines))),
                   {'schema_version':1,'usable':False,'facts':{}}]
        expected=[evaluate(g,s) for s in states]
        script="const p=JSON.parse(require('fs').readFileSync(0,'utf8'));process.stdout.write(JSON.stringify(p.states.map(s=>require('./src/hoi4companion/web/guide-runtime.js').evaluate(p.guide,s))));"
        result=subprocess.run(['node','-e',script],input=json.dumps({'guide':g,'states':states}),text=True,capture_output=True,check=True)
        self.assertEqual(json.loads(result.stdout),expected)

    def test_explicit_overlapping_layout_or_obsolete_all_target_does_not_silently_reuse_completion(self):
        g,s=document({'category':'production','production_targets':[
            {'equipment_id':'inf','requested_factories':1},
            {'equipment_id':'inf','quantity_mode':'remaining','production_category':'military'}]})
        report=preview(g,s,CATALOG)
        self.assertEqual(report['operations'][0]['reason'],'production_targets_invalid')
        self.assertFalse(report['operations'][0]['automatic_completion'])
        g,s=document({'category':'production','equipment_id':'inf','quantity_mode':'remaining','production_category':'military'})
        compiled=preview(g,s,CATALOG)['guide']
        params=compiled['tracks']['operations'][0]['action']['params']
        params.pop('quantity_mode');params['requested_factories']=5
        numeric=preview(compiled,s,CATALOG)['guide']['tracks']['operations'][0]
        self.assertEqual(numeric['completion'],{'op':'fact','key':'guide.production.order.requested','test':'eq','value':5})
