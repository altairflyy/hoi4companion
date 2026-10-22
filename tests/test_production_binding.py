import copy
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from threading import RLock, Thread
from types import SimpleNamespace
from unittest.mock import patch
from http.server import ThreadingHTTPServer
from urllib.request import Request, urlopen

from hoi4companion.contracts import normalise_state
from hoi4companion.guide_runtime import evaluate
from hoi4companion.guide_progress import RunStore
from hoi4companion.operation_import import operation_catalog
from hoi4companion.guide_importer import preview
from test_operation_import import document
from hoi4companion.api import Store, make_handler
from hoi4companion.country import localise_country

CASES = json.loads((Path(__file__).parent / 'fixtures/production-progress.json').read_text())


class ProductionBindingTests(unittest.TestCase):
    def test_type_localisation_keeps_the_custom_model_and_native_identity(self):
        original=copy.deepcopy(CASES[0]['state']['sample']['country_overview'])
        result=localise_country(original,{'tool_1':'Attrezzi di base','TST_tool_1':'Modello nazionale'})
        row=result['production']['lines'][0]
        self.assertEqual(row['equipment_name'],'Attrezzi di base')
        self.assertEqual(row['equipment'],'tool_1');self.assertEqual(row['name'],'Model A')
        self.assertNotIn('equipment_name',original['production']['lines'][0])

    def test_shared_runtime_fixtures_and_purity(self):
        for case in CASES:
            with self.subTest(case=case['name']):
                before = copy.deepcopy(case)
                row = evaluate(case['guide'], case['state'], case['progress'])['steps'][0]
                self.assertEqual(row['status'], case['expected'])
                self.assertEqual(row['production']['assigned'], case['assigned'])
                self.assertEqual(case, before)

    def test_equipment_catalog_uses_native_families_and_country_names(self):
        with TemporaryDirectory() as directory:
            root = Path(directory); (root/'common/units/equipment').mkdir(parents=True)
            (root/'common/units/equipment/example.txt').write_text('equipments={ tool={is_archetype=yes} tool_1={archetype=tool} tool_2={archetype=tool} }')
            entries, warnings = operation_catalog(root, [], {'tool':'Attrezzi','TST_tool_1':'Modello locale'},
                                                  {'tool':'Tools','tool_1':'Basic Tools','TST_tool_1':'National Model'})
            self.assertEqual(warnings, [])
            entries = {e['id']:e for e in entries if e['kind']=='equipment'}
            self.assertEqual(entries['tool']['equipment_ids'], ['tool_1','tool_2'])
            self.assertIn('National Model', entries['tool_1']['country_aliases']['TST'])
            self.assertIn('Modello locale', entries['tool_1']['country_aliases']['TST'])
            guide,sources = document({'category':'production','equipment_name':'Tools','assigned_factories':3})
            report = preview(guide,sources,list(entries.values()))
            self.assertEqual(report['operations'][0]['monitoring'],'automatic')
            selector=report['guide']['tracks']['operations'][0]['action']['params']['production_selector']
            self.assertEqual(selector['equipment_ids'], ['tool_1','tool_2'])
            guide['tracks']['operations'][0]['action']['params']['variant_name']='National Model'
            selector=preview(guide,sources,list(entries.values()))['guide']['tracks']['operations'][0]['action']['params']['production_selector']
            self.assertEqual(selector['equipment_ids'],['tool_1'])

    def test_ambiguous_equipment_and_invalid_existing_ids_do_not_fall_back(self):
        catalog=[{'kind':'equipment','id':i,'name':'Tools','aliases':[], 'equipment_ids':[i]} for i in ('one','two')]
        guide,sources=document({'category':'production','equipment_name':'Tools','assigned_factories':3})
        report=preview(guide,sources,catalog)
        self.assertEqual(report['operations'][0]['reason'],'production_equipment_ambiguous')
        self.assertNotIn('completion', report['guide']['tracks']['operations'][0])
        guide['tracks']['operations'][0]['action']['params']['equipment_id']='invalid'
        self.assertEqual(preview(guide,sources,catalog)['operations'][0]['reason'],'production_equipment_unresolved')

    def test_declared_manual_criteria_and_missing_quantities_are_preserved(self):
        catalog=[{'kind':'equipment','id':'tool','name':'Tools','aliases':[], 'equipment_ids':['tool']}]
        guide,sources=document({'category':'production','equipment_name':'Tools','assigned_factories':3})
        guide['tracks']['operations'][0]['completion']={'op':'check','key':'author-check'}
        report=preview(guide,sources,catalog)
        self.assertEqual(report['guide']['tracks']['operations'][0]['completion'],{'op':'check','key':'author-check'})
        self.assertEqual(report['operations'][0]['monitoring'],'start_only')
        del guide['tracks']['operations'][0]['completion']
        guide['tracks']['operations'][0]['action']['params']['assigned_factories']=None
        report=preview(guide,sources,catalog)
        self.assertEqual(report['operations'][0]['reason'],'production_target_missing')
        self.assertEqual(report['operations'][0]['monitoring'],'start_only')

    def test_revalidation_removes_obsolete_generated_selectors_and_conditions(self):
        catalog=[{'kind':'equipment','id':'tool','name':'Tools','aliases':[], 'equipment_ids':['tool']}]
        guide,sources=document({'category':'production','equipment_name':'Tools','assigned_factories':3})
        compiled=preview(guide,sources,catalog)['guide']
        self.assertEqual(preview(compiled,sources,catalog)['guide'],compiled)
        compiled['tracks']['operations'][0]['action']['params']['equipment_name']='Unknown'
        report=preview(compiled,sources,catalog);step=report['guide']['tracks']['operations'][0]
        self.assertEqual(report['operations'][0]['monitoring'],'manual')
        self.assertNotIn('production_selector',step['action']['params'])
        self.assertNotIn('completion',step)

    def test_unique_custom_variant_pairs_are_reused_but_conflicts_are_not_guessed(self):
        catalog=[{'kind':'equipment','id':key,'name':name,'aliases':[], 'equipment_ids':[key]}
                 for key,name in [('tool','Tools'),('other','Other Tools')]]
        guide,sources=document({'category':'production','equipment_name':'Tools','variant_name':'Custom Model','assigned_factories':1})
        second=copy.deepcopy(guide['tracks']['operations'][0]);second['key']='later'
        second['action']['params']={'category':'production','equipment_name':'Custom Model','assigned_factories':3}
        guide['tracks']['operations'].append(second)
        report=preview(guide,sources,catalog)
        self.assertEqual(report['operations'][1]['monitoring'],'automatic')
        selector=report['guide']['tracks']['operations'][1]['action']['params']['production_selector']
        self.assertEqual(selector['variant_names'],['Custom Model'])
        conflict=copy.deepcopy(guide['tracks']['operations'][0]);conflict['key']='conflict'
        conflict['action']['params']['equipment_name']='Other Tools';guide['tracks']['operations'].append(conflict)
        self.assertEqual(preview(guide,sources,catalog)['operations'][1]['reason'],'production_equipment_ambiguous')

    def test_additional_assignment_captures_run_local_initial_count(self):
        case=copy.deepcopy(CASES[0]); guide=case['guide']; step=guide['tracks']['operations'][0]
        step['action']['params']['quantity_mode']='additional'
        step['completion']={'op':'fact','key':'guide.production.order.added','test':'gte','value':3}
        step['started_when']={'op':'fact','key':'guide.production.order.started','test':'eq','value':True}
        line=case['state']['sample']['country_overview']['production']['lines'][0]
        line.update(requested_factories=2,assigned_factories=2,active_factories=2)
        runs=RunStore(id_factory=lambda:'r');runs.create('production','session')
        self.assertTrue(runs.observe(guide,case['state']))
        self.assertEqual(runs.get('production')['baselines']['order']['value'],2)
        for assigned,requested,expected in [(2,2,'ready'),(2,5,'active'),(4,5,'active'),(5,5,'completed'),(2,2,'ready')]:
            line.update(assigned_factories=assigned,active_factories=assigned,requested_factories=requested)
            self.assertFalse(runs.observe(guide,case['state']))
            self.assertEqual(evaluate(guide,case['state'],runs.get('production'))['steps'][0]['status'],expected)

    def test_projection_requires_complete_qualified_consistent_lines(self):
        sample=copy.deepcopy(CASES[0]['state'])
        self.assertNotEqual(normalise_state(sample)['facts']['production.lines']['status'],'unknown')
        for change in ('partial','foreign','malformed','duplicate','stale'):
            value=copy.deepcopy(sample);overview=value['sample']['country_overview'];section=overview['production']
            if change=='partial':section['status']='partial'
            elif change=='foreign':overview['country']='OTHER'
            elif change=='malformed':section['lines'][0]['assigned_factories']=99
            elif change=='duplicate':section['lines'].append(copy.deepcopy(section['lines'][0]))
            else:value['usable']=False
            self.assertEqual(normalise_state(value)['facts']['production.lines']['status'],'unknown',change)

    def test_http_runtime_and_payload_use_the_same_assignment_counters(self):
        case=copy.deepcopy(CASES[0]);guide,live=case['guide'],case['state'];live['sample']['focus_ids']=[]
        store=Store.__new__(Store);store.lock=RLock();store.settings={'country':'TST','game_dir':'fixture'}
        store.default_guide_id='production';store.guide_checks={};store.guide_states={};store.guide_revision=0
        store.trees={};store.labels={};store.technologies={'technologies':{},'folders':{}};store.demo=False
        store.live_monitor=SimpleNamespace(poll=lambda:copy.deepcopy(live));store.live_provider_path='fixture'
        store.live_provider=None;store.live_cache=copy.deepcopy(live);store.live_cache_time=0
        with patch('hoi4companion.api.load_guide',return_value=guide):
            with ThreadingHTTPServer(('127.0.0.1',0),make_handler(store)) as server:
                thread=Thread(target=server.serve_forever,daemon=True);thread.start()
                def call(path):
                    with urlopen(f'http://127.0.0.1:{server.server_port}'+path,timeout=3) as response:return json.load(response)
                try:
                    for assigned,requested,expected in [(1,3,'active'),(3,3,'completed'),(0,0,'ready')]:
                        live['sample']['country_overview']['production']['lines'][0].update(assigned_factories=assigned,requested_factories=requested,active_factories=assigned)
                        store.live_cache_time=0
                        result=call('/api/guide/runtime')['steps'][0]
                        self.assertEqual(result['status'],expected)
                        self.assertEqual(result['production']['assigned'],assigned)
                        state=call('/api/state')
                        self.assertEqual(state['sample']['country_overview']['production']['lines'][0]['assigned_factories'],assigned)
                    with urlopen(f'http://127.0.0.1:{server.server_port}/production-progress.js') as response:
                        self.assertIn(b'HoiProductionProgress',response.read())
                finally:server.shutdown();thread.join(timeout=2)
