"""Run-local construction baselines; no native acquisition or game writes."""
import copy
import json
from pathlib import Path
import unittest
from threading import RLock, Thread
from types import SimpleNamespace
from unittest.mock import patch
from http.server import ThreadingHTTPServer
from urllib.request import urlopen, Request
from urllib.error import HTTPError

from hoi4companion.guide_progress import RunStore
from hoi4companion.guide_runtime import evaluate
from hoi4companion.api import Store, make_handler


CASES = json.loads((Path(__file__).parent / 'fixtures' / 'construction-progress.json').read_text())


class ConstructionProgressTests(unittest.TestCase):
    def test_shared_relative_construction_fixtures(self):
        for case in CASES:
            with self.subTest(case=case['name']):
                before = copy.deepcopy(case)
                row = evaluate(case['guide'], case['state'], case['progress'])['steps'][0]
                self.assertEqual(row['status'], case['expected'])
                self.assertEqual(row['completion']['value'], case['completed'])
                self.assertEqual(case, before)

    def test_capture_once_cancel_partial_complete_and_isolate_runs(self):
        case = CASES[0]; guide = case['guide']; state = copy.deepcopy(case['state'])
        ids = iter(['r1', 'r2', 'r3']); runs = RunStore(id_factory=lambda: next(ids))
        runs.create('build', 'session')
        self.assertTrue(runs.observe(guide, state))
        first = runs.get('build')
        self.assertEqual(first['baselines']['factory']['value'], 5)
        self.assertNotIn('baselines', runs.metadata()['build'])
        for level, queued, expected in [(5, [], 'ready'), (5, ['state:77:arms_factory'], 'active'),
                                         (5, [], 'ready'), (6, ['state:77:arms_factory'], 'active'),
                                         (6, [], 'ready'), (7, [], 'completed')]:
            state['facts']['country.states.items.77.military_factories']['value'] = level
            state['facts']['construction.queued']['value'] = queued
            self.assertFalse(runs.observe(guide, state))
            self.assertEqual(evaluate(guide, state, runs.get('build'))['steps'][0]['status'], expected)
            self.assertEqual(runs.get('build')['baselines']['factory']['value'], 5)
        first['baselines']['factory']['value'] = 0
        self.assertEqual(runs.get('build')['baselines']['factory']['value'], 5)
        runs.create('build', 'session')
        self.assertNotIn('baselines', runs.get('build'))
        self.assertTrue(runs.observe(guide, state))
        self.assertEqual(runs.get('build')['baselines']['factory']['value'], 7)
        self.assertEqual(runs.get('build', 'r1')['baselines']['factory']['value'], 5)
        runs.bind('other-session')
        self.assertNotIn('baselines', runs.get('build'))

    def test_no_capture_from_unknown_stale_wrong_country_session_or_already_queued(self):
        case = CASES[0]
        for change in ('stale', 'country', 'session', 'unknown', 'queued', 'calculated', 'missing_country'):
            runs = RunStore(id_factory=lambda: 'r'); runs.create('build', 'session')
            state = copy.deepcopy(case['state'])
            if change == 'stale': state['usable'] = False
            elif change == 'country': state['facts']['country']['value'] = 'OTHER'
            elif change == 'missing_country': del state['facts']['country']
            elif change == 'session': state['session_id'] = 'other'
            elif change == 'queued': state['facts']['construction.queued']['value'] = ['state:77:arms_factory']
            else: state['facts']['country.states.items.77.military_factories']['status'] = change
            self.assertFalse(runs.observe(case['guide'], state), change)
            self.assertNotIn('baselines', runs.get('build'))

    def test_dependency_and_conditions_delay_capture_and_manual_writes_preserve_it(self):
        case = copy.deepcopy(CASES[0]); guide = case['guide']
        guide['tracks']['operations'].insert(0, {'key': 'gate', 'action': {'kind': 'manual'}})
        guide['tracks']['operations'][1]['depends_on'] = ['gate']
        runs = RunStore(id_factory=lambda: 'r'); runs.create('build', 'session')
        self.assertFalse(runs.observe(guide, case['state']))
        runs.update('build', {'gate': True})
        self.assertTrue(runs.observe(guide, case['state']))
        runs.update('build', {'gate': False}, started={'gate': True})
        self.assertEqual(runs.get('build')['baselines']['factory']['value'], 5)
        guide['tracks']['operations'][1]['when'] = {'op': 'check', 'key': 'allow'}
        runs = RunStore(id_factory=lambda: 'r'); runs.create('build', 'session')
        runs.update('build', {'gate': True})
        self.assertFalse(runs.observe(guide, case['state']))

    def test_malformed_baselines_are_rejected(self):
        case = CASES[0]
        for baselines in (None, [], {'factory': {'value': True}}, {'factory': {'value': -1}}):
            with self.subTest(value=baselines), self.assertRaises(ValueError):
                evaluate(case['guide'], case['state'], {**case['progress'], 'baselines': baselines})

    def test_two_orders_for_the_same_counter_do_not_share_the_same_increment(self):
        case = copy.deepcopy(CASES[0]); guide, state = case['guide'], case['state']
        second = copy.deepcopy(guide['tracks']['operations'][0]); second['key'] = 'second'
        second['completion']['key'] = 'guide.construction.second.added'
        guide['tracks']['operations'].append(second)
        runs = RunStore(id_factory=lambda: 'r'); runs.create('build', 'session')
        self.assertTrue(runs.observe(guide, state))
        self.assertEqual(set(runs.get('build')['baselines']), {'factory'})
        state['facts']['country.states.items.77.military_factories']['value'] = 7
        self.assertTrue(runs.observe(guide, state))
        self.assertEqual(runs.get('build')['baselines']['second']['value'], 7)
        self.assertEqual([row['status'] for row in evaluate(guide, state, runs.get('build'))['steps']], ['completed', 'ready'])

    def test_http_and_live_event_payload_share_the_server_owned_baseline(self):
        guide = CASES[0]['guide']
        live = {'usable': True, 'sample': {'country': 'TST', 'session_id': 'session', 'focus_ids': [],
                'country_overview': {'country': 'TST', 'states': {'status': 'observed',
                    'items': [{'id': 77, 'military_factories': 5}]},
                    'construction': {'status': 'observed', 'lines': []}}}}
        store = Store.__new__(Store); store.lock = RLock(); store.settings = {'game_dir': 'fixture', 'country': 'TST'}
        store.default_guide_id = 'build'; store.guide_checks = {}; store.guide_states = {}; store.guide_revision = 0
        store.labels = {}; store.trees = {}; store.technologies = {'technologies': {}, 'folders': {}}; store.demo = False
        store.live_monitor = SimpleNamespace(poll=lambda: copy.deepcopy(live)); store.live_provider_path = 'fixture'
        store.live_provider = None; store.live_cache = copy.deepcopy(live); store.live_cache_time = 0
        with patch('hoi4companion.api.load_guide', return_value=guide):
            with ThreadingHTTPServer(('127.0.0.1', 0), make_handler(store)) as server:
                thread = Thread(target=server.serve_forever, daemon=True); thread.start()
                def call(path, payload=None):
                    request = Request(f'http://127.0.0.1:{server.server_port}' + path,
                        data=None if payload is None else json.dumps(payload).encode(), headers={'Content-Type': 'application/json'})
                    with urlopen(request, timeout=3) as response: return json.load(response)
                try:
                    progress = call('/api/guide/progress')
                    self.assertEqual(progress['baselines']['factory']['value'], 5)
                    for count, amount, expected in [(5, 2, 'active'), (5, 0, 'ready'), (6, 1, 'active'), (7, 0, 'completed')]:
                        overview = live['sample']['country_overview']
                        overview['states']['items'][0]['military_factories'] = count
                        overview['construction']['lines'] = ([{'id':'line','building_id':'arms_factory','state_id':77,
                            'province_id':None,'amount':amount,'active_factories':0,'built_level':count,
                            'repair_levels':0,'conversion':False,'location_status':'observed'}] if amount else [])
                        store.live_cache_time = 0
                        row = call('/api/guide/runtime')['steps'][0]
                        self.assertEqual(row['status'], expected)
                        self.assertEqual(row['construction']['target'], 7)
                        event = store.live_state()
                        self.assertEqual(event['guide_baselines']['build']['factory']['value'], 5)
                        self.assertEqual(event['guide_runs']['build']['run_id'], progress['run_id'])
                    with self.assertRaises(HTTPError):
                        call('/api/guide/progress', {'run_id': progress['run_id'], 'baselines': {}})
                    call('/api/guide/progress', {})
                    self.assertEqual(call('/api/guide/progress')['baselines']['factory']['value'], 7)
                finally:
                    server.shutdown(); thread.join(timeout=2)
