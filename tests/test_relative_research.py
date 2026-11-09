import copy
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from hoi4companion.guide_progress import RunStore
from hoi4companion.guide_runtime import evaluate
from hoi4companion.guide_importer import preview
from hoi4companion.import_sources import acquire_source
from hoi4companion.relative_research import project, adapt_legacy
from hoi4companion.screen_overlay import targets


def fixture():
    source = acquire_source(text='Artillery UP\nArtillery UP')
    catalog = [dict(kind='technology', id=k, name=n, upgrade_successors=s, aliases=[])
               for k,n,s in [('a','Artillery I',['b']),('b','Artillery II',['c']),('c','Artillery III',[])]]
    rows = [dict(key='r'+str(i), action={'kind':'research','entity':{'kind':'technology','id':'Artillery UP'}},
                 evidence=[{'source_id':source['id'],'basis':'explicit','quote':'Artillery UP'}]) for i in range(2)]
    guide = {'schema_version':1,'id':'relative-test','title':'Relative', 'applies_to':{'country':'POL'},
             'sources':[{'id':source['id'],'type':'manual'}], 'tracks':{'research':rows}}
    report = preview(guide,[source],catalog)
    assert report['valid'], report['errors']
    return report['guide']


def state(completed, active=(), *, country='POL', usable=True):
    def fact(value): return {'status':'observed','source':'fixture','value':value}
    return {'schema_version':1,'usable':usable,'session_id':'one','facts':{'country':fact(country),
            'research.completed':fact(list(completed)), 'research.active':fact(list(active)),
            'research.slots':fact([{'technology_id':None}])}}


class RelativeResearchTests(unittest.TestCase):
    def test_python_javascript_progression_contract_matches(self):
        guide=fixture();guide['tracks']['research'].append({'key':'other','action':{'kind':'research','entity':{'kind':'technology','id':'d'}}})
        progress={'schema_version':1,'guide_id':guide['id'],'run_id':'x','session_id':'one','checks':{}}
        cases=[{'guide':guide,'state':live,'progress':{**progress,'research_bindings':bindings}} for live,bindings in [
            (state(['a'],['b']),{'r0':'b'}),(state(['a']),{'r0':'c'}),
            (state(['a','b','c']),{}),(state(['a'],usable=False),{'r0':'b'})]]
        script="const fs=require('node:fs'),r=require(process.argv[1]);process.stdout.write(JSON.stringify(JSON.parse(fs.readFileSync(0,'utf8')).map(x=>r.evaluate(x.guide,x.state,x.progress))));"
        result=subprocess.run(['node','-e',script,str(Path(__file__).resolve().parents[1]/'src/hoi4companion/web/guide-runtime.js')],input=json.dumps(cases),text=True,capture_output=True,check=True)
        self.assertEqual(json.loads(result.stdout),[evaluate(x['guide'],x['state'],x['progress']) for x in cases])

    def test_rewind_keeps_campaign_targets_without_copying_completion_or_manual_checks(self):
        guide=fixture(); store=RunStore(); store.ensure(guide['id'])
        campaign='hoi4:11111111-1111-1111-1111-111111111111:00000001'
        def live(done):
            value=state(done);value['facts']['campaign.id']={'status':'observed','source':'fixture','value':campaign}
            return value
        store.bind_campaign(campaign,'POL','one',100)
        store.observe(guide,live(['a']))
        store.update(guide['id'],{'manual':True})
        store.bind_campaign(campaign,'POL','one',200)
        store.observe(guide,live(['a','b']))
        store.bind_campaign(campaign,'POL','one',150)
        store.observe(guide,live(['a','b']))
        progress=store.get(guide['id'])
        self.assertEqual(progress['checks'],{})
        self.assertEqual(progress['research_bindings'],{'r0':'b','r1':'c'})
        self.assertEqual([r['status'] for r in evaluate(guide,live(['a','b']),progress)['steps']],['completed','ready'])
        store.bind_campaign(campaign,'POL','one',50)
        store.observe(guide,live(['a']))
        result=evaluate(guide,live(['a']),store.get(guide['id']))
        self.assertEqual([r['status'] for r in result['steps']],['ready','waiting'])

    def test_unbound_later_up_does_not_hide_an_independent_research(self):
        guide=fixture(); guide['tracks']['research'].append({'key':'other','action':{'kind':'research','entity':{'kind':'technology','id':'d'}}})
        store=RunStore();store.create(guide['id'],'one');store.observe(guide,state(['a']))
        result=evaluate(guide,state(['a'],['b']),store.get(guide['id']))
        self.assertEqual(result['steps'][1]['status'],'waiting')
        self.assertEqual(result['parallel']['research']['steps'],['other'])

    def test_finished_chain_does_not_request_another_level_or_block_other_research(self):
        guide=fixture();guide['tracks']['research'].append({'key':'other','action':{'kind':'research','entity':{'kind':'technology','id':'d'}}})
        store=RunStore();store.create(guide['id'],'one');store.observe(guide,state(['a','b','c']))
        result=evaluate(guide,state(['a','b','c']),store.get(guide['id']))
        self.assertEqual([r['status'] for r in result['steps']],['not_applicable','not_applicable','ready'])
        self.assertEqual(result['parallel']['research']['steps'],['other'])

    def test_bound_future_level_waits_for_lower_levels_but_native_selection_wins(self):
        guide=fixture();progress={'schema_version':1,'guide_id':guide['id'],'run_id':'x','session_id':'one','checks':{},'research_bindings':{'r0':'c'}}
        result=evaluate(guide,state(['a']),progress)
        self.assertEqual(result['steps'][0]['status'],'waiting')
        self.assertEqual(result['parallel']['research']['steps'],[])
        self.assertEqual(evaluate(guide,state(['a'],['c']),progress)['steps'][0]['status'],'active')

    def test_legacy_rewind_repairs_shifted_bindings_from_the_same_campaign_and_persists(self):
        guide=fixture(); campaign='hoi4:11111111-1111-1111-1111-111111111111:00000001'
        live=state(['a']);live['facts']['campaign.id']={'status':'observed','source':'fixture','value':campaign}
        with tempfile.TemporaryDirectory() as root:
            path=Path(root)/'runs.sqlite3';store=RunStore(path=path);store.ensure(guide['id'])
            store.bind_campaign(campaign,'POL','one',100);store.observe(guide,live)
            store.bind_campaign(campaign,'POL','one',50)
            # Persisted pre-fix rewind: target b was forgotten and r0 rebound c.
            snapshot=store.snapshot()
            record=snapshot['runs'][store.get(guide['id'])['run_id']]
            record.pop('research_history');record['research_bindings']={'r0':'c'}
            store._load(snapshot)
            live['facts']['research.completed']['value']=['a','b']
            self.assertTrue(store.observe(guide,live))
            self.assertEqual(store.get(guide['id'])['research_bindings'],{'r0':'b','r1':'c'})
            store=RunStore(path=path)
            self.assertEqual(store.get(guide['id'])['research_history'],{'r0':'b'})
            self.assertFalse(store.observe(guide,live))

    def test_history_never_crosses_campaign_or_rebinds_without_qualified_live_evidence(self):
        guide=fixture();store=RunStore();store.ensure(guide['id'])
        first='hoi4:11111111-1111-1111-1111-111111111111:00000001'
        second='hoi4:22222222-2222-2222-2222-222222222222:00000001'
        live=state(['a']);live['facts']['campaign.id']={'status':'observed','source':'fixture','value':first}
        store.bind_campaign(first,'POL','one',100);store.observe(guide,live)
        store.bind_campaign(second,'POL','one',100);live['facts']['campaign.id']['value']=second
        live['facts']['research.completed']['value']=['a','b'];store.observe(guide,live)
        self.assertEqual(store.get(guide['id'])['research_bindings'],{'r0':'c'})
        store.bind_campaign(first,'POL','one',50);live['facts']['campaign.id']['value']=first
        before=store.get(guide['id']);bad=copy.deepcopy(live);bad['usable']=False
        self.assertFalse(store.observe(guide,bad));self.assertEqual(store.get(guide['id']),before)
        store.observe(guide,live);self.assertEqual(store.get(guide['id'])['research_bindings'],{'r0':'b','r1':'c'})

    def test_naval_shell_shorthand_advances_from_observed_research_not_battery_paths(self):
        source=acquire_source(text='Armor Piercing…(navy)')
        definition={'id':'naval-shells','title':'Naval Shells','sources':[{'id':source['id'],'type':'manual'}],
            'tracks':{'research':[{'key':'shell','action':{'kind':'research','entity':{'kind':'technology','id':source['text']}},
                'evidence':[{'source_id':source['id'],'basis':'explicit','quote':source['text']}]}]}}
        catalog=[{'kind':'technology','id':key,'name':name,'upgrade_successors':links} for key,name,links in [
            ('basic_heavy_shell','Armor-Piercing Capped Shell',['heavy_battery']),
            ('improved_heavy_shell','Super-Heavy Armor Piercing Shell',[]),('heavy_battery','Heavy Battery',[])]]
        report=preview(definition,[source],catalog)
        self.assertTrue(report['valid'],report['errors'])
        run=RunStore();run.create('naval-shells','one')
        self.assertTrue(run.observe(report['guide'],state(['basic_heavy_shell'])))
        self.assertEqual(run.get('naval-shells')['research_bindings'],{'shell':'improved_heavy_shell'})
    def test_first_up_binds_first_unresearched_and_does_not_move_after_completion(self):
        guide=fixture(); store=RunStore();store.create(guide['id'],'one')
        self.assertTrue(store.observe(guide,state(['a'])))
        self.assertEqual(store.get(guide['id'])['research_bindings'],{'r0':'b'})
        self.assertFalse(store.observe(guide,state(['a'],['b'])))
        self.assertTrue(store.observe(guide,state(['a','b'])))
        progress=store.get(guide['id'])
        self.assertEqual(progress['research_bindings'],{'r0':'b','r1':'c'})
        self.assertEqual([x['status'] for x in evaluate(guide,state(['a','b']),progress)['steps']],['completed','ready'])

    def test_previously_unbound_projection_can_become_a_real_candidate(self):
        guide=project(fixture(),{})
        store=RunStore();store.create(guide['id'],'one')
        self.assertTrue(store.observe(guide,state(['a'])))
        self.assertEqual(store.get(guide['id'])['research_bindings'],{'r0':'b'})

    def test_first_up_can_be_first_level_and_unknown_or_foreign_evidence_cannot_bind(self):
        guide=fixture(); store=RunStore();store.create(guide['id'],'one')
        self.assertFalse(store.observe(guide,state([],country='GER')))
        missing=state([]); del missing['facts']['research.completed']
        self.assertFalse(store.observe(guide,missing))
        self.assertTrue(store.observe(guide,state([])))
        self.assertEqual(store.get(guide['id'])['research_bindings'],{'r0':'a'})

    def test_declared_next_upgrade_does_not_require_up_in_the_label(self):
        src=acquire_source(text='Artillery')
        guide={'schema_version':1,'id':'declared','title':'Declared','sources':[{'id':src['id'],'type':'manual'}],
               'tracks':{'research':[{'key':'r','action':{'kind':'research','entity':{'kind':'technology','id':'Artillery'},
                 'params':{'interpretation':'next_upgrade'}},'evidence':[{'source_id':src['id'],'basis':'explicit','quote':'Artillery'}]}]}}
        catalog=[{'kind':'technology','id':'a','name':'Artillery I','aliases':[],'upgrade_successors':['b']},
                 {'kind':'technology','id':'b','name':'Artillery II','aliases':[],'upgrade_successors':[]}]
        result=preview(guide,[src],catalog)
        self.assertTrue(result['valid'],result['errors'])
        self.assertEqual(result['guide']['tracks']['research'][0]['relative_research']['chain'],['a','b'])

    def test_bindings_persist_and_are_isolated_by_run_and_stale_session(self):
        guide=fixture()
        with tempfile.TemporaryDirectory() as root:
            path=Path(root)/'runs.sqlite3'; store=RunStore(path=path);first=store.create(guide['id'],'one')
            store.observe(guide,state(['a'])); store=RunStore(path=path)
            self.assertEqual(store.get(guide['id'])['research_bindings'],{'r0':'b'})
            changed=state(['a','b']);changed['session_id']='other'
            result=evaluate(guide,changed,store.get(guide['id']))
            self.assertIn(result['progress_status'],('wrong_session','campaign_unverified'))
            self.assertTrue(all(row['status']!='completed' for row in result['steps']))
            store.create(guide['id'],'one');self.assertNotIn('research_bindings',store.get(guide['id']))
            self.assertEqual(store.get(guide['id'],first['run_id'])['research_bindings'],{'r0':'b'})

    def test_inactive_guide_is_not_bound_and_ambiguous_branch_is_not_guessed(self):
        guide=fixture(); store=RunStore();store.create(guide['id'],'one')
        self.assertFalse(store.observe(guide,state([]),allow_research=False))
        guide['tracks']['research'][0]['relative_research']['chain']=['a','a']
        with self.assertRaises(ValueError): evaluate(guide,state([]),store.get(guide['id']))

    def test_campaign_switch_and_timeline_rewind_do_not_reuse_the_old_binding(self):
        guide=fixture();store=RunStore();store.ensure(guide['id'])
        first='hoi4:11111111-1111-1111-1111-111111111111:00000001'
        other='hoi4:22222222-2222-2222-2222-222222222222:00000002'
        def campaign_state(campaign, completed):
            value=state(completed);value['facts']['campaign.id']={'status':'observed','source':'fixture','value':campaign}
            return value
        store.bind_campaign(first,'POL','one',100);self.assertTrue(store.observe(guide,campaign_state(first,['a'])))
        old=store.get(guide['id']);self.assertEqual(old['research_bindings']['r0'],'b')
        store.bind_campaign(other,'POL','one',100);self.assertTrue(store.observe(guide,campaign_state(other,[])))
        self.assertEqual(store.get(guide['id'])['research_bindings']['r0'],'a')
        store.bind_campaign(first,'POL','one',100);self.assertEqual(store.get(guide['id'])['research_bindings']['r0'],'b')
        store.bind_campaign(first,'POL','one',50)
        self.assertNotIn('research_bindings',store.get(guide['id']))

    def test_hud_consumes_the_same_bound_native_target_as_runtime(self):
        guide=fixture();store=RunStore();store.create(guide['id'],'one')
        live={'usable':True,'sample':{'session_id':'one','country':'POL','researched_technologies':['a'],
                                      'research_slots':[{'technology_id':None}]}}
        self.assertTrue(store.observe(guide,live))
        progress=store.get(guide['id']);bound=project(guide,progress['research_bindings'])
        result=evaluate(bound,live,progress)
        catalog=[{'kind':'technology','id':'b','name':'Artillery II','aliases':[]}]
        shown=targets(bound,result,catalog,live)
        self.assertEqual([(row['key'],row['identity']) for row in shown],[('r0','b')])
        self.assertEqual(targets(bound,result,catalog,{'usable':False}),[])

    def test_runtime_and_hud_expose_the_bound_id_even_with_a_cached_guide(self):
        guide=fixture();store=RunStore();store.create(guide['id'],'one')
        live={'usable':True,'sample':{'session_id':'one','country':'POL','researched_technologies':['a'],
                                    'research_slots':[{'technology_id':None}]}}
        store.observe(guide,live);result=evaluate(guide,live,store.get(guide['id']))
        self.assertEqual(result['steps'][0]['research_target'],'b')
        catalog=[{'kind':'technology','id':'b','name':'Artillery II','aliases':[]}]
        self.assertEqual([p['identity'] for p in targets(guide,result,catalog,live)],['b'])

    def test_legacy_upgrade_adapts_without_mutating_original_and_missing_branch_is_unknown(self):
        guide=fixture(); row=guide['tracks']['research'][0]
        row.pop('relative_research');row['action']['entity']['id']='b';row['target']='b'
        row['research_upgrade']={'query':'Artillery UP','from_id':'a','basis':'country_history_and_catalog_paths'}
        before=copy.deepcopy(guide)
        catalog={'a':{'id':'a','name':'Artillery I','aliases':[],'upgrade_successors':['b']},
                 'b':{'id':'b','name':'Artillery II','aliases':[],'upgrade_successors':['c']},
                 'c':{'id':'c','name':'Artillery III','aliases':[],'upgrade_successors':[]}}
        adapted=adapt_legacy(guide,catalog)
        self.assertEqual(guide,before)
        self.assertEqual(adapted['tracks']['research'][0]['relative_research']['chain'],['a','b','c'])
        store=RunStore();store.create(guide['id'],'one');store.observe(adapted,state([]))
        self.assertEqual(store.get(guide['id'])['research_bindings']['r0'],'a')
        missing=adapt_legacy(guide,{})
        self.assertTrue(missing['tracks']['research'][0]['relative_research_unresolved'])
        self.assertNotEqual(evaluate(missing,state(['b']),store.get(guide['id']))['steps'][0]['status'],'completed')


if __name__ == '__main__': unittest.main()
