"""Province fort goals use built levels, never disappearance from the queue."""
import copy
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch
from PIL import Image

from hoi4companion.fortifications import command, bind, context, enrich_borders
from hoi4companion.map_topology import load_neighbors
from hoi4companion.contracts import normalise_state
from hoi4companion.guide_runtime import evaluate
from hoi4companion.country import CountryReader
from hoi4companion.live import LiveReadError, compile_watch_reads, watch_changed
from test_construction import ConstructionFixture


CATALOG = [{'kind':'building','id':'bunker','name':'Land Fort','location_scope':'province','max_level':10},
           {'kind':'building','id':'coastal_bunker','name':'Coastal Fort','location_scope':'province','max_level':10}]


def guide():
    row = {'key':'fort','action':{'kind':'manual','params':{'category':'construction',
        'building_id':'bunker','target_level':1,'province_selector':{'kind':'border','country':'YUG'}}}}
    bind(row, CATALOG)
    return {'id':'frontier','title':'Border','schema_version':1,'dependency_policy':'explicit',
            'applies_to':{'country':'HUN'},'tracks':{'operations':[row]}}


def state():
    def fact(value, status='observed'):
        return {'value':value,'status':status,'source':'fixture'}
    return {'schema_version':1,'usable':True,'facts':{'country':fact('HUN'),
        'fortifications.borders.YUG':fact([10,11], 'calculated'),
        'country.fortifications.items.10.bunker':fact(0),
        'country.fortifications.items.11.bunker':fact(0),
        'construction.queued':fact([], 'calculated')}}


class FortificationTests(unittest.TestCase):
    def test_shared_runtime_fixtures(self):
        cases=json.loads((Path(__file__).parent/'fixtures/fortifications.json').read_text())
        for case in cases:
            with self.subTest(case=case['name']):
                row=evaluate(case['guide'],case['state'])['steps'][0]
                self.assertEqual(row['status'],case['expected'])
                self.assertEqual(row['fortifications']['target_level'],1)
                self.assertNotIn('construction',row)

    def test_existing_hungarian_style_instruction_is_promoted_without_rewriting_file(self):
        from hoi4companion.informal_operations import promote_operations
        from hoi4companion.operation_import import bind_operations
        from hoi4companion.resolution import catalog_index
        text='BUILD: 1 fort in each tile at YUG border, 1 INFA in N-Hungary'
        g={'id':'any','dependency_policy':'explicit','applies_to':{'country':'HUN'},'tracks':{'operations':[
            {'key':'build','tracking':'reminder','action':{'kind':'manual','params':{'category':'checkpoints','instructions':[text]}}}]}}
        index=catalog_index(CATALOG)
        promote_operations(g,index);reports=bind_operations(g,index)
        rows=g['tracks']['operations']
        self.assertEqual(len(rows),2)
        self.assertNotIn('tracking',rows[0])
        self.assertEqual(rows[0]['action']['params']['province_selector']['country'],'YUG')
        self.assertEqual(rows[1]['action']['params']['instructions'],['1 INFA in N-Hungary'])
        self.assertEqual(reports[0]['monitoring'],'automatic')
        before=copy.deepcopy(g);promote_operations(g,index);bind_operations(g,index)
        self.assertEqual(g,before)

    def test_old_import_substep_keys_and_existing_notes_are_preserved(self):
        from hoi4companion.informal_operations import promote_operations
        from hoi4companion.operation_import import bind_operations
        from hoi4companion.resolution import catalog_index
        text='BUILD: 1 fort in each tile at YUG border, 1 INFA in N-Hungary'
        reminder=lambda key,text:{'key':key,'tracking':'reminder','action':{'kind':'manual','params':{'category':'checkpoints','instructions':[text]}}}
        g={'id':'any','dependency_policy':'explicit','applies_to':{'country':'HUN'},'tracks':{'operations':[
            reminder('old',text),reminder('old--operation-2','Unrelated existing instruction'),reminder('old--note-1','Existing note')]}}
        original=copy.deepcopy(g['tracks']['operations'][1:]);index=catalog_index(CATALOG)
        promote_operations(g,index);bind_operations(g,index)
        rows=g['tracks']['operations']
        self.assertNotIn('tracking',rows[0])
        self.assertEqual(rows[1]['key'],'old--note-2')
        self.assertEqual([row['key'] for row in rows[2:]],[row['key'] for row in original])
        self.assertEqual(rows[2]['action']['params']['instructions'],['Unrelated existing instruction'])

    def test_conditional_fortification_preserves_authored_gate(self):
        from hoi4companion.informal_operations import promote_operations
        from hoi4companion.operation_import import bind_operations
        from hoi4companion.resolution import catalog_index
        g=guide();row=g['tracks']['operations'][0]
        row.pop('completion');row.pop('started_when');row['tracking']='reminder'
        row['when']={'op':'fact','key':'campaign.ready','test':'eq','value':True}
        row['action']['params']={'category':'checkpoints','instructions':['After preparation: 1 fort in each tile at YUG border']}
        index=catalog_index(CATALOG);promote_operations(g,index);bind_operations(g,index)
        self.assertNotIn('tracking',g['tracks']['operations'][0])
        s=state();s['facts']['campaign.ready']={'status':'observed','source':'fixture','value':False}
        self.assertEqual(evaluate(g,s)['steps'][0]['status'],'waiting')
        s['facts']['campaign.ready']['value']=True
        self.assertEqual(evaluate(g,s)['steps'][0]['status'],'ready')

    def test_new_authoring_location_variants_compile_to_existing_schema(self):
        from hoi4companion.guide_authoring import compile_document
        for scope in ({'border':'YUG'},{'provinces':[10,11]},{'province':10}):
            step={'key':'f','type':'build','building':{'id':'bunker'},'quantity':{'mode':'total','value':1},**scope}
            step['evidence']=[{'source_id':'s','basis':'explicit','quote':'Build forts.'}]
            result=compile_document({'format':'hoi4-guide-authoring','authoring_schema_version':1,
                'guide':{'id':'g','title':'Generic fort','steps':[step]},
                'sources':[{'id':'s','type':'manual','text':'Build forts.','segments':[]}]})
            # Compiler output is the unchanged Guide Schema v1, not a new runtime.
            self.assertIn('guide',result)

    def test_untrusted_derived_completion_cannot_override_missing_levels(self):
        g,s=guide(),state()
        s['facts']['guide.fortifications.fort.completed']={'status':'observed','source':'external','value':True}
        self.assertEqual(evaluate(g,s)['steps'][0]['status'],'ready')

    def test_bad_selector_is_unknown_and_never_crashes(self):
        for value in (None,5,{},True):
            g=guide();g['tracks']['operations'][0]['action']['params']['province_selector']['country']=value
            self.assertEqual(evaluate(g,state())['steps'][0]['status'],'unverifiable')

    def test_provincial_group_is_not_silently_reinterpreted_as_a_factory_order(self):
        from hoi4companion.operation_import import bind_operations
        g=guide();p=g['tracks']['operations'][0]['action']['params']
        row=g['tracks']['operations'][0];row.pop('completion');row.pop('started_when')
        p['building_id']='arms_factory'
        reports=bind_operations(g,[*CATALOG,{'kind':'building','id':'arms_factory','name':'Factory','location_scope':'state'}])
        self.assertEqual(reports[0]['monitoring'],'manual')
        self.assertNotIn('started_when',row)

    def test_additional_province_levels_reuse_campaign_baselines(self):
        from hoi4companion.guide_progress import RunStore
        from hoi4companion.operation_import import bind_operations
        g=guide();row=g['tracks']['operations'][0];p=row['action']['params']
        for key in ('completion','started_when'):row.pop(key)
        p.pop('province_selector');p.pop('target_level');p.update(province_id=10,quantity=2,quantity_mode='additional')
        entries=[*CATALOG,{'kind':'state','id':'1','name':'Region','geography':{'province_ids':[10],'cities':[]}}]
        bind_operations(g,entries)
        s=state();s['session_id']='session';s['facts']['country.fortifications.items.10.bunker']['value']=3
        runs=RunStore(id_factory=lambda:'r');runs.create(g['id'],'session')
        self.assertTrue(runs.observe(g,s))
        self.assertEqual(runs.get(g['id'])['baselines']['fort']['value'],3)
        s['facts']['construction.queued']['value']=['province:10:bunker']
        self.assertEqual(evaluate(g,s,runs.get(g['id']))['steps'][0]['status'],'active')
        s['facts']['construction.queued']['value']=[]
        self.assertEqual(evaluate(g,s,runs.get(g['id']))['steps'][0]['status'],'ready')
        s['facts']['country.fortifications.items.10.bunker']['value']=5
        result=evaluate(g,s,runs.get(g['id']))['steps'][0]
        self.assertEqual(result['status'],'completed')
        self.assertEqual(result['fortifications']['target_level'],5)

    def test_native_levels_and_border_contract_rejects_duplicates_and_bad_counts(self):
        section={'status':'observed','items':[{'id':10,'bunker':1},{'id':11,'bunker':1}],
                 'borders_status':'observed','borders':{'YUG':[10,11]},'controllers':{'10':'HUN'}}
        wire={'usable':True,'sample':{'country':'HUN','country_overview':{'country':'HUN','fortifications':section}}}
        self.assertEqual(evaluate(guide(),wire)['steps'][0]['status'],'completed')
        self.assertNotIn('country.fortifications.controllers.10',normalise_state(wire)['facts'])
        section['items'].append({'id':10,'bunker':1})
        self.assertNotEqual(evaluate(guide(),wire)['steps'][0]['status'],'completed')
        section['items'].pop();section['items'][0]['bunker']=True
        self.assertNotEqual(evaluate(guide(),wire)['steps'][0]['status'],'completed')
    def test_border_command_and_residual_instruction_are_preserved(self):
        result=command('BUILD: 1 fort in each tile at YUG border, 1 INFA in N-Hungary',CATALOG)
        self.assertEqual(result['params']['province_selector'],{'kind':'border','country':'YUG'})
        self.assertEqual(result['params']['target_level'],1)
        self.assertEqual(result['remainder'],'1 INFA in N-Hungary')
        self.assertIsNone(command('Maybe build some forts near the enemy',CATALOG))

    def test_runtime_queue_partial_cancel_and_complete(self):
        g,s=guide(),state()
        before=copy.deepcopy((g,s))
        self.assertEqual(evaluate(g,s)['steps'][0]['status'],'ready')
        self.assertEqual((g,s),before)
        s['facts']['construction.queued']['value']=['province:10:bunker']
        self.assertEqual(evaluate(g,s)['steps'][0]['status'],'active')
        s['facts']['country.fortifications.items.10.bunker']['value']=1
        self.assertNotEqual(evaluate(g,s)['steps'][0]['status'],'completed')
        s['facts']['construction.queued']['value']=[]
        self.assertEqual(evaluate(g,s)['steps'][0]['status'],'ready')
        s['facts']['country.fortifications.items.11.bunker']['value']=1
        self.assertEqual(evaluate(g,s)['steps'][0]['status'],'completed')

    def test_unknown_empty_border_foreign_queue_and_stale_do_not_complete(self):
        for change in ('missing_level','missing_border','empty','wrong_country','stale','foreign_queue'):
            g,s=guide(),state()
            if change=='missing_level':del s['facts']['country.fortifications.items.11.bunker']
            if change=='missing_border':del s['facts']['fortifications.borders.YUG']
            if change=='empty':s['facts']['fortifications.borders.YUG']['value']=[]
            if change=='wrong_country':s['facts']['country']['value']='GER'
            if change=='stale':s['usable']=False
            if change=='foreign_queue':s['facts']['construction.queued']['value']=['province:99:bunker']
            row=evaluate(g,s)['steps'][0]
            self.assertNotIn(row['status'],('completed','active'),change)

    def test_explicit_provinces_use_each_level_and_do_not_accept_a_state_total(self):
        g=guide();p=g['tracks']['operations'][0]['action']['params']
        del p['province_selector'];p['province_ids']=[10,11];p['target_level']=4
        s=state();s['facts']['country.fortifications.items.10.bunker']['value']=4
        s['facts']['country.fortifications.items.11.bunker']['value']=3
        self.assertNotEqual(evaluate(g,s)['steps'][0]['status'],'completed')
        s['facts']['country.fortifications.items.11.bunker']['value']=4
        self.assertEqual(evaluate(g,s)['steps'][0]['status'],'completed')

    def test_topology_joins_observed_controllers_and_rejects_missing_edges(self):
        raw={'status':'observed','items':[{'id':10,'bunker':0},{'id':11,'bunker':0}],
             'controllers':{'10':'HUN','11':'HUN','20':'YUG','30':'AUS'}}
        neighbors={10:[11,20],11:[10,30]}
        result=enrich_borders(raw,neighbors,'HUN')
        self.assertEqual(result['borders'],{'AUS':[11],'YUG':[10]})
        self.assertNotIn('controllers',result)
        self.assertEqual(enrich_borders(raw,{10:[20]},'HUN')['borders_status'],'unavailable')
        self.assertNotIn('borders',raw)

    def test_repairs_and_conversions_are_not_new_fort_orders(self):
        base={'id':'q','building_id':'bunker','location_status':'observed','state_id':1,
              'province_id':10,'amount':1,'active_factories':0,'built_level':0,'repair_levels':0,'conversion':False}
        for repair,conversion,expected in [(0,False,['province:10:bunker']),(1,False,[]),(0,True,[])]:
            row={**base,'repair_levels':repair,'conversion':conversion}
            wire={'usable':True,'sample':{'country':'HUN','country_overview':{'country':'HUN',
                'construction':{'status':'observed','lines':[row]}}}}
            keys=normalise_state(wire)['facts']['construction.queued']['value']
            self.assertEqual([k for k in keys if k.startswith('province:')],expected)


class MapTopologyTests(unittest.TestCase):
    def test_pixel_land_edges_and_impassable_overrides(self):
        with TemporaryDirectory() as folder:
            game=Path(folder);(game/'map').mkdir()
            (game/'map/definition.csv').write_text('1;255;0;0;land;false;plains;1\n2;0;255;0;land;false;plains;1\n3;0;0;255;sea;true;ocean;0\n')
            image=Image.new('RGB',(3,2));image.putdata([(255,0,0),(0,255,0),(0,0,255)]*2)
            image.save(game/'map/provinces.bmp')
            neighbors=load_neighbors(game,[])
            self.assertEqual(neighbors,{1:[2],2:[1]})
            (game/'map/adjacencies.csv').write_text('From;To;Type\n1;2;impassable\n')
            self.assertEqual(load_neighbors(game,[]),{1:[],2:[]})


class ProvincialLiveTests(unittest.TestCase):
    def test_fortification_probe_budget_cannot_break_other_live_sections(self):
        from hoi4companion.country import collect_country
        from test_native_fixture import Fixture
        f=Fixture();f.profile.country_sections=('production','fortifications')
        def oversized(reader):
            for index in range(513):
                address=0x400000000+index*4096
                reader.watch[address,1]=b'a';reader.watch[address+4095,1]=b'b'
            return {'status':'observed','items':[],'controllers':{}}
        plan={}
        with patch.object(CountryReader,'production',return_value={'status':'observed','lines':[]}),patch.object(CountryReader,'fortifications',oversized):
            result=collect_country(f,f.world,f.country,1,'ROM',plan)
        self.assertEqual(result['production']['status'],'observed')
        self.assertEqual(result['fortifications']['status'],'unavailable')
        compile_watch_reads(plan)

    def test_unqualified_game_profiles_do_not_inherit_provincial_offsets(self):
        from hoi4companion.live import EXECUTABLE_PROFILES,MILITARY_HASH,UPDATED_HASH
        for digest in (MILITARY_HASH,UPDATED_HASH):
            self.assertNotIn('fortifications',EXECUTABLE_PROFILES[digest]['country_sections'])

    def test_large_country_reads_only_fort_slots_within_existing_budgets(self):
        f=self.fixture()
        # Other building kinds must not inflate each province's sample/watch.
        f.array(f.status+0x20,[0]+[-1]*255,4)
        provinces=[0,0,f.province]
        for identifier in range(3,4099):
            province=f.object('CProvince')
            f.put(province+0xA4,'<i',identifier)
            f.put(province+0xC0,'<Q',f.state)
            f.put(province+0x188,'<i',1)
            status=province+0x190
            f.put(status,'<Q',int.from_bytes(f.read(f.status,8),'little'))
            f.put(status+0x68,'<iii',1,identifier,0)
            f.array(status+0x20,[-1]*256,4)
            f.array(status+0x38,[])
            provinces.append(province)
        f.array(f.world+0x2B0,provinces)
        reader=CountryReader(f,f.world,f.country,1)
        value=reader.fortifications()
        reader.s.verify()
        self.assertEqual(len(value['items']),4097)
        self.assertEqual(value['items'][-1]['bunker'],0)
        self.assertLess(reader.s.used,2*1024*1024)
        self.assertLess(len(reader.s.witnesses),131072)
        compile_watch_reads(reader.watch)

    def test_fort_map_membership_change_is_detected_without_other_slots(self):
        f=self.fixture();f.array(f.status+0x20,[-1]*256,4)
        reader=CountryReader(f,f.world,f.country,1)
        self.assertEqual(reader.fortifications()['items'][0]['bunker'],0)
        plan=compile_watch_reads(reader.watch)
        f.put(reader.u(f.status+0x20),'<i',0)
        self.assertTrue(watch_changed(f,plan))
    def fixture(self):
        f=ConstructionFixture(provincial=True)
        f.string(f.definition+0x270,'bunker')
        return f

    def test_levels_survive_queue_removal_and_are_watched(self):
        f=self.fixture();f.array(f.production+0x70,[])
        r=CountryReader(f,f.world,f.country,1)
        value=r.fortifications();r.s.verify()
        self.assertEqual(value['items'][0]['bunker'],3)
        plan=compile_watch_reads(r.watch)
        f.put(f.building+0x40,'<h',4)
        self.assertTrue(watch_changed(f,plan))

    def test_reciprocal_identity_and_explicit_absence(self):
        f=self.fixture();f.array(f.status+0x20,[-1],4);f.array(f.status+0x38,[])
        self.assertEqual(CountryReader(f,f.world,f.country,1).fortifications()['items'][0]['bunker'],0)
        f.put(f.status+0x6C,'<i',99)
        with self.assertRaises(LiveReadError):CountryReader(f,f.world,f.country,1).fortifications()
