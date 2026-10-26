import copy
import json
from pathlib import Path
import subprocess
import unittest

from hoi4companion.action_readiness import hud_resource_ready
from hoi4companion.action_resources import enrich_resource_costs, project_costs
from hoi4companion.clausewitz import Node
from hoi4companion.contracts import normalise_state
from hoi4companion.country import CountryReader
from hoi4companion.guide import normalise_guide
from hoi4companion.guide_runtime import evaluate
from hoi4companion.live import NativeSample, compile_watch_reads, watch_changed
from hoi4companion.screen_overlay import targets
from test_guide_runtime import state
from test_operational_improvements import ProjectFixture
from test_politics_native import PoliticsFixture


POINT='projects.points.specialization_land'


def project():
    return {'id':'resources','tracks':{'operations':[{'key':'project','depends_on':[],
        'resource_costs':{POINT:1},'action':{'kind':'special_project','entity':{'kind':'project','id':'flame'}},
        'started_when':{'op':'fact','key':'projects.active','test':'contains','value':'flame'},
        'completion':{'op':'fact','key':'projects.completed','test':'contains','value':'flame'}}]}}


class ActionResourceTests(unittest.TestCase):
    def project_fixture(self,units):
        f=ProjectFixture();r=CountryReader(f,f.world,f.country,1)
        obj=f.object('CBreakthroughProgress@NProject')
        f.put(f.status+0x50,'<Q',NativeSample(f).number(obj))
        head=f.allocate();node=f.allocate()
        f.put(f.status+0x58,'<QQ',head,1)
        f.put(head+8,'<Q',node);f.put(head+0x19,'<B',1)
        f.put(node,'<QQQ',head,head,head);f.put(node+0x19,'<B',0)
        f.put(node+0x1C,'<ii',123,units)
        r.token_name=lambda token:'specialization_land'
        return f,r,node

    def test_fractional_breakthrough_progress_never_rounds_up_to_spendable_points(self):
        for units,points in [(0,0),(9999,0),(10000,1),(19999,1),(20000,2)]:
            with self.subTest(units=units):
                f,r,node=self.project_fixture(units)
                row=r.projects()['breakthroughs']['items'][0];r.s.verify()
                self.assertEqual(row['available'],points)
                self.assertEqual(row['progress'],units%10000/10000)
                plan=compile_watch_reads(r.watch);f.put(node+0x20,'<i',units+1)
                self.assertTrue(watch_changed(f,plan))

    def test_invalid_tree_preserves_project_membership_but_does_not_publish_points(self):
        for mutate in [lambda f,node:f.put(node,'<Q',node),
                       lambda f,node:f.put(node+8,'<Q',node),
                       lambda f,node:f.put(f.status+0x60,'<Q',2),
                       lambda f,node:f.put(node+0x20,'<i',-1)]:
            f,r,node=self.project_fixture(10000);mutate(f,node)
            value=r.projects()
            self.assertEqual(value['status'],'observed')
            self.assertEqual(value['breakthroughs']['status'],'unavailable')
            self.assertNotIn('items',value['breakthroughs'])

    def test_political_power_precision_negative_values_and_watch_plan(self):
        for amount in (14999999,15000000,-500000):
            f=PoliticsFixture();f.put(f.manager+0xE0,'<q',amount);r=f.reader()
            self.assertEqual(r.politics()['power']['available'],amount/100000)
            plan=compile_watch_reads(r.watch);f.put(f.manager+0xE0,'<q',amount+1)
            self.assertTrue(watch_changed(f,plan))

    def test_catalog_costs_are_literal_scoped_and_do_not_modify_the_authored_guide(self):
        node=Node([('breakthrough_cost',Node([('specialization_land','1'),('specialization_naval','2')]))])
        self.assertEqual(project_costs(node),{POINT:1,'projects.points.specialization_naval':2})
        for value in ('-1','@scripted_cost',Node([('value','1')])):
            self.assertIsNone(project_costs(Node([('breakthrough_cost',Node([('specialization_land',value)]))])))
        guide=project();guide['tracks']['operations'][0].pop('resource_costs');before=copy.deepcopy(guide)
        enriched=enrich_resource_costs(copy.deepcopy(guide),[{'kind':'project','id':'flame','resource_costs':{POINT:1}}])
        self.assertEqual(enriched['tracks']['operations'][0]['resource_costs'],{POINT:1})
        self.assertEqual(guide,before)

    def test_hud_waits_for_sufficient_known_points_and_respects_unknown_costs(self):
        guide=project();catalog=[{'kind':'project','id':'flame','name':'Flame Tank','specialization':'specialization_land'}]
        raw={'usable':True,'sample':{'country':'GER','session_id':'session'}}
        for amount,expected in [(0,False),(1,True),(2,True),(None,False),(True,False)]:
            live=state(**{'projects.active':[],'projects.completed':[],POINT:amount})
            runtime=evaluate(guide,live)
            self.assertEqual(bool(targets(guide,runtime,catalog,raw)),expected)
        guide['tracks']['operations'][0].pop('resource_costs')
        runtime=evaluate(guide,state(**{'projects.active':[],'projects.completed':[],POINT:10}))
        self.assertFalse(targets(guide,runtime,catalog,raw))
        self.assertIn('project_cost_unknown',runtime['steps'][0]['action_readiness']['unknown'])

    def test_expensive_first_political_choice_is_never_replaced_with_a_cheaper_successor(self):
        guide={'id':'resources','tracks':{'operations':[
            {'key':key,'depends_on':[],'political_power_cost':cost,'action':{'kind':'manual','params':{
                'category':'politics','law_id':key,'law_group':'economy'}},
             'completion':{'op':'fact','key':'politics.ideas','test':'contains','value':key}}
            for key,cost in [('first',150),('second',25)]]}}
        catalog=[{'kind':'law','id':key,'name':key,'group':'economy'} for key in ('first','second')]
        raw={'usable':True,'sample':{'country':'GER','session_id':'session'}}
        for amount,keys in [(149.99999,[]),(150,['first']),(None,[])]:
            live=state(**{'politics.ideas':[],'country.politics.political_power':amount})
            runtime=evaluate(guide,live)
            self.assertEqual(runtime['action_sequence']['political'],'first')
            self.assertEqual([r['key'] for r in targets(guide,runtime,catalog,raw)],keys)
        live=state(**{'politics.ideas':['first'],'country.politics.political_power':30})
        self.assertEqual([r['key'] for r in targets(guide,evaluate(guide,live),catalog,raw)],['second'])

    def test_python_javascript_resource_readiness_parity_and_strict_contract(self):
        examples=[]
        for amount in (0,1,2,None,True):
            examples.append({'guide':project(),'state':state(**{'projects.active':[],'projects.completed':[],POINT:amount})})
        script="""const fs=require('fs'),r=require('./src/hoi4companion/web/guide-runtime.js');
        process.stdout.write(JSON.stringify(JSON.parse(fs.readFileSync(0,'utf8')).map(x=>r.evaluate(x.guide,x.state))));"""
        actual=json.loads(subprocess.check_output(['node','-e',script],input=json.dumps(examples).encode(),cwd=Path(__file__).resolve().parents[1]))
        self.assertEqual(actual,[evaluate(x['guide'],x['state']) for x in examples])
        for cost in (True,-1,'1',float('inf')):
            guide=project();guide['tracks']['operations'][0]['resource_costs']={POINT:cost}
            with self.assertRaises(ValueError):normalise_guide(guide)

    def test_stale_foreign_or_partial_balances_remain_unknown_in_both_contract_adapters(self):
        value={'usable':True,'sample':{'country':'GER','country_overview':{'country':'GER',
            'projects':{'status':'observed','items':[],'breakthroughs':{'status':'observed','items':[{'id':'specialization_land','available':1}]}},
            'politics':{'power':{'status':'observed','available':150.25}}}}}
        variants=[value]
        for change in ('foreign','stale','partial','fractional'):
            v=copy.deepcopy(value)
            if change=='foreign':v['sample']['country_overview']['country']='POL'
            elif change=='stale':v['usable']=False
            elif change=='fractional':v['sample']['country_overview']['projects']['breakthroughs']['items'][0]['available']=0.9
            else:v['sample']['country_overview']['projects']['breakthroughs']['status']='partial'
            variants.append(v)
        script="""const fs=require('fs'),c=require('./src/hoi4companion/web/contracts.js');
        process.stdout.write(JSON.stringify(JSON.parse(fs.readFileSync(0,'utf8')).map(x=>{const s=c.state(x);return ['projects.points.specialization_land','country.politics.political_power'].map(k=>c.fact(s,k).value)})));"""
        actual=json.loads(subprocess.check_output(['node','-e',script],input=json.dumps(variants).encode(),cwd=Path(__file__).resolve().parents[1]))
        expected=[[normalise_state(v)['facts'].get(k,{}).get('value') for k in [POINT,'country.politics.political_power']] for v in variants]
        self.assertEqual(actual,expected)
        self.assertEqual(actual[0],[1,150.25]);self.assertEqual(actual[1],[None,None])


if __name__=='__main__':unittest.main()
