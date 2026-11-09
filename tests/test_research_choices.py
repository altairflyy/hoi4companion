import copy
import tempfile
import unittest
import json
import subprocess
from pathlib import Path

from hoi4companion.guide_importer import preview
from hoi4companion.guide_progress import RunStore
from hoi4companion.guide_runtime import evaluate
from hoi4companion.import_sources import acquire_source
from hoi4companion.screen_overlay import targets


def fixture():
    names=['Concentrated/dispersed Industry 1','Concentrated/dispersed UP','Concentrated UP']
    src=acquire_source(text='\n'.join(names))
    catalog=[{'kind':'technology','id':branch+str(level),'name':name+' Industry '+str(level),
              'upgrade_successors':[branch+str(level+1)] if level<3 else []}
             for branch,name in [('c','Concentrated'),('d','Dispersed')] for level in range(1,4)]
    guide={'schema_version':1,'id':'choice','title':'Choice','applies_to':{'country':'FRA'},
           'sources':[{'id':src['id'],'type':'manual'}],
           'tracks':{'research':[{'key':'r'+str(i),'action':{'kind':'research','entity':{'kind':'technology','id':name}},
                     'evidence':[{'source_id':src['id'],'basis':'explicit','quote':name}]} for i,name in enumerate(names)]}}
    report=preview(guide,[src],catalog)
    assert report['valid'],report['errors']
    return report['guide'],catalog


def state(completed=(),active=(),country='FRA'):
    fact=lambda value:{'status':'observed','source':'fixture','value':value}
    return {'schema_version':1,'usable':True,'session_id':'s','facts':{'country':fact(country),
            'campaign.id':fact('hoi4:11111111-1111-1111-1111-111111111111:00000001'),
            'research.completed':fact(list(completed)),'research.active':fact(list(active)),
            'research.slots':fact([{'technology_id':None}]*3)}}


class ResearchChoiceTests(unittest.TestCase):
    def test_mixed_one_off_alternative_does_not_choose_from_old_completed_industry_or_bind_later_tools(self):
        from hoi4companion.research_upgrades import resolve_research_upgrades
        guide,catalog=fixture();catalog += [{'kind':'technology','id':'turbine','name':'High-Efficiency Turbines','upgrade_successors':[]},
                                          {'kind':'technology','id':'tool1','name':'Basic Machine Tools','upgrade_successors':['tool2']},
                                          {'kind':'technology','id':'tool2','name':'Improved Machine Tools','upgrade_successors':[]}]
        guide['tracks']['research']=copy.deepcopy(guide['tracks']['research'][:2])
        for row,query in zip(guide['tracks']['research'],['High Efficiency Turbines/Concentrated UP','Tools UP']):
            row.pop('research_choice',None);row['action']['entity']['id']=query
        resolve_research_upgrades(guide,catalog)
        self.assertEqual(guide['tracks']['research'][0]['research_choice']['mode'],'once')
        self.assertNotIn('research_choice',guide['tracks']['research'][1])
        store=RunStore();store.create('choice','s')
        live=state(['c1'])
        self.assertTrue(store.observe(guide,live))
        progress=store.get('choice')
        self.assertNotIn('r0',progress.get('research_bindings',{}))
        self.assertEqual(set(progress['research_options']['r0']),{'turbine','c2'})
        result=evaluate(guide,live,progress)
        self.assertEqual(set(result['steps'][0]['research_options']),{'turbine','c2'})
        # A change between samples is still recognized from the frozen offer.
        self.assertTrue(store.observe(guide,state(['c1','c2'])))
        self.assertEqual(store.get('choice')['research_bindings']['r0'],'c2')
        self.assertEqual(evaluate(guide,state(['c1','c2']),store.get('choice'))['steps'][0]['status'],'completed')

    def test_python_javascript_runtime_parity_for_open_selected_missing_and_conflicting_evidence(self):
        guide,_=fixture()
        progress={'schema_version':1,'guide_id':'choice','run_id':'run','session_id':'s','checks':{}}
        cases=[{'guide':guide,'state':state(),'progress':progress},
               {'guide':guide,'state':state(['d1']),'progress':{**progress,'research_bindings':{'r0':'d1','r1':'d2'}}},
               {'guide':guide,'state':state(['c1','d1']),'progress':progress},
               {'guide':guide,'state':state(),'progress':{**progress,'session_id':'other'}}]
        missing=state();missing['facts']['research.completed']={'status':'unknown','value':None,'source':''}
        cases.append({'guide':guide,'state':missing,'progress':progress})
        reordered=copy.deepcopy(guide)
        reordered['tracks']['research'][0]['research_choice']['branches']=dict(reversed(list(reordered['tracks']['research'][0]['research_choice']['branches'].items())))
        cases.append({'guide':reordered,'state':state(),'progress':progress})
        once=copy.deepcopy(guide);once['tracks']['research']=once['tracks']['research'][:1]
        once['tracks']['research'][0]['research_choice']={'group':'step:r0','query':'Turbines/Concentrated UP','mode':'once',
            'branches':{'industry':['c1','c2','c3'],'turbines':['turbine']}}
        cases.extend([{'guide':once,'state':state(['c1']),'progress':progress},
                      {'guide':once,'state':state(['c1','c2']),'progress':{**progress,'research_options':{'r0':['c2','turbine']}}}])
        busy=copy.deepcopy(once);busy['tracks']['research'].insert(0,{'key':'prior','action':{'kind':'research','entity':{'kind':'technology','id':'c1'}},'relative_research':{'query':'Concentrated UP','chain':['c1','c2','c3']}})
        cases.append({'guide':busy,'state':state(['c1'],['c2']),'progress':{**progress,'research_bindings':{'prior':'c2'}}})
        self.assertEqual(evaluate(busy,cases[-1]['state'],cases[-1]['progress'])['steps'][1]['research_options'],['turbine'])
        module=(Path(__file__).parent.parent/'src/hoi4companion/web/guide-runtime.js').as_posix()
        script='const r=require(process.argv[1]);let s="";process.stdin.on("data",x=>s+=x);process.stdin.on("end",()=>process.stdout.write(JSON.stringify(JSON.parse(s).map(x=>r.evaluate(x.guide,x.state,x.progress)))));'
        result=subprocess.run(['node','-e',script,module],input=json.dumps(cases),capture_output=True,text=True,encoding='utf8',check=True)
        self.assertEqual(json.loads(result.stdout),[evaluate(x['guide'],x['state'],x['progress']) for x in cases])

    def test_invalid_choice_chains_or_levels_are_rejected(self):
        from hoi4companion.guide import normalise_guide
        guide,_=fixture()
        for change in (lambda c:c['branches'].update(dispersed=['c1']),lambda c:c.update(level=4),lambda c:c.update(group=''),lambda c:c.update(mode='once')):
            broken=copy.deepcopy(guide);change(broken['tracks']['research'][0]['research_choice'])
            with self.assertRaises(ValueError):normalise_guide(broken)

    def test_reimport_cannot_retain_a_branch_absent_from_the_current_catalog(self):
        from hoi4companion.research_upgrades import resolve_research_upgrades
        guide,catalog=fixture()
        guide['tracks']['research'][0]['research_choice']['query']='Unknown explicit branch choice'
        with self.assertRaises(ValueError):resolve_research_upgrades(guide,[x for x in catalog if x['id']!='d3'])

    def test_frozen_one_off_targets_survive_restart_and_do_not_leak_into_a_new_run(self):
        from hoi4companion.research_upgrades import resolve_research_upgrades
        guide,catalog=fixture();guide['tracks']['research']=guide['tracks']['research'][:1]
        row=guide['tracks']['research'][0];row.pop('research_choice');row['action']['entity']['id']='High Efficiency Turbines/Concentrated UP'
        catalog.append({'kind':'technology','id':'turbine','name':'High-Efficiency Turbines','upgrade_successors':[]})
        resolve_research_upgrades(guide,catalog)
        campaign='hoi4:11111111-1111-1111-1111-111111111111:00000001'
        with tempfile.TemporaryDirectory() as root:
            path=Path(root)/'runs.sqlite';store=RunStore(path=path);store.create('choice','s');store.bind_campaign(campaign,'FRA','s',100)
            store.observe(guide,state(['c1']));self.assertEqual(set(store.get('choice')['research_options']['r0']),{'c2','turbine'})
            store=RunStore(path=path);self.assertTrue(store.observe(guide,state(['c1','c2'])))
            self.assertEqual(store.get('choice')['research_bindings']['r0'],'c2')
            store.create('choice','s');self.assertNotIn('research_options',store.get('choice'))

    def test_open_choice_is_two_options_for_one_step_not_two_independent_research_orders(self):
        guide,catalog=fixture();before=copy.deepcopy(guide)
        store=RunStore();store.create('choice','s')
        live=state();self.assertFalse(store.observe(guide,live))
        result=evaluate(guide,live,store.get('choice'))
        self.assertEqual(result['parallel']['research']['steps'],['r0'])
        self.assertEqual(result['steps'][0]['research_options'],['c1','d1'])
        raw={**live,'sample':{'country':'FRA','session_id':'s'}}
        proposals=targets(guide,result,catalog,raw)
        self.assertEqual({p['identity'] for p in proposals},{'c1','d1'})
        self.assertTrue(all(p['tone']=='choice' for p in proposals))
        self.assertEqual(len({p['key'] for p in proposals}),2)
        self.assertEqual(guide,before)

    def test_observed_selection_locks_subsequent_upgrades_to_that_branch_and_survives_restart(self):
        guide,_=fixture()
        with tempfile.TemporaryDirectory() as root:
            path=Path(root)/'progress.sqlite';store=RunStore(path=path);store.create('choice','s')
            store.bind_campaign('hoi4:11111111-1111-1111-1111-111111111111:00000001','FRA','s',100)
            self.assertTrue(store.observe(guide,state(active=['d1'])))
            self.assertEqual(store.get('choice')['research_bindings'],{'r0':'d1'})
            self.assertEqual(evaluate(guide,state(active=['d1']),store.get('choice'))['steps'][0]['status'],'active')
            store=RunStore(path=path)
            self.assertTrue(store.observe(guide,state(['d1'])))
            self.assertEqual(store.get('choice')['research_bindings'],{'r0':'d1','r1':'d2'})
            self.assertTrue(store.observe(guide,state(['d1','d2'])))
            self.assertEqual(store.get('choice')['research_bindings']['r2'],'d3')
            store.create('choice','s');self.assertNotIn('research_bindings',store.get('choice'))

    def test_unknown_foreign_conflicting_or_stale_observations_do_not_choose(self):
        guide,_=fixture();store=RunStore();store.create('choice','s')
        for live in [state(country='GER'),state(['c1','d1']),{**state(active=['c1']),'session_id':'other'}]:
            self.assertFalse(store.observe(guide,live))
            self.assertFalse(evaluate(guide,live,store.get('choice'))['parallel']['research']['steps'])
        live=state();del live['facts']['research.completed']
        self.assertFalse(store.observe(guide,live))
        self.assertFalse(evaluate(guide,live,store.get('choice'))['parallel']['research']['steps'])

    def test_source_conditions_and_dependencies_are_not_bypassed_by_a_choice(self):
        guide,_=fixture();guide['tracks']['research'][0]['when']={'op':'check','key':'gate'}
        store=RunStore();store.create('choice','s')
        result=evaluate(guide,state(),store.get('choice'))
        self.assertFalse(result['parallel']['research']['steps'])
        self.assertFalse(store.observe(guide,state(['d1'])))

