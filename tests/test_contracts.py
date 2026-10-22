import json
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
import unittest

from hoi4companion.contracts import fact, normalise_state, validate_condition, validate_action, evaluate_condition
from hoi4companion.guide import load_guide, list_guides, normalise_guide
from unittest.mock import patch
from threading import RLock


class ContractTests(unittest.TestCase):
    def test_unknown_is_not_false_or_zero_and_calculation_has_provenance(self):
        self.assertEqual(fact(None)['status'], 'unknown')
        self.assertEqual(fact(0, 'observed', 'process_memory')['value'], 0)
        self.assertEqual(fact(False, 'observed', 'process_memory')['value'], False)
        with self.assertRaises(ValueError): fact(3, 'unknown')
        with self.assertRaises(ValueError): fact(3, 'calculated', '')

    def test_stale_sample_and_incomplete_membership_never_become_facts(self):
        sample = {'country': 'GER', 'focus_tree_id': 'g', 'completed_focuses': ['A'],
                  'researched_technologies': ['t'], 'research_slots': [], 'focus_complete': False}
        state = normalise_state({'usable': True, 'source': 'process_memory', 'sample': sample})
        self.assertEqual(state['schema_version'], 1)
        self.assertEqual(state['facts']['focus.completed']['status'], 'unknown')
        self.assertEqual(state['facts']['country']['value'], 'GER')
        self.assertEqual(normalise_state({'usable': False, 'sample': sample})['facts']['country']['status'], 'unknown')

    def test_statistical_derivations_and_unknown_fields_remain_distinct(self):
        sample = {'country': 'ROM', 'country_overview': {'country': 'ROM',
                  'production': {'status': 'observed', 'lines': [{'id': '1','output_per_day': 2.5,'assigned_factories': 3}]},
                  'manpower': {'status': 'partial', 'available': 0, 'training': None}}}
        state = normalise_state({'usable': True, 'sample': sample})
        self.assertEqual(state['facts']['country.manpower.available']['status'], 'calculated')
        self.assertEqual(state['facts']['country.manpower.training']['status'], 'unknown')
        self.assertEqual(state['facts']['country.production.lines.1.output_per_day']['status'], 'calculated')

    def test_conditions_and_actions_reject_unbounded_or_executable_input(self):
        validate_condition({'op': 'all', 'args': [{'op': 'fact', 'key': 'country', 'test': 'eq', 'value': 'AUS'}]})
        with self.assertRaises(ValueError): validate_condition({'op': 'eval', 'code': 'anything'})
        with self.assertRaises(ValueError): validate_action({'kind': 'run_command', 'command': 'anything'})
        with self.assertRaises(ValueError): validate_action({'kind': 'research', 'entity': {'kind': 'technology','id': ''}})

    def test_catalog_discovers_guides_without_a_country_specific_loader(self):
        with TemporaryDirectory() as root:
            for key, country in [('alpha', 'AUS'), ('beta', 'GER')]:
                data = {'schema': 1, 'id': key, 'country': country, 'tree_id': 'tree',
                        'tracks': {'focus': [{'key': 'first', 'kind': 'focus', 'target': 'A'}], 'research': []}}
                Path(root, key + '.json').write_text(json.dumps(data), encoding='utf-8')
            self.assertEqual([g['id'] for g in list_guides(Path(root))], ['alpha', 'beta'])
            self.assertEqual(load_guide('beta', directory=Path(root))['country'], 'GER')
            with self.assertRaises(ValueError): load_guide('../beta', directory=Path(root))

    def test_legacy_migration_preserves_order_and_declares_generic_conditions(self):
        legacy = load_guide()
        result = normalise_guide(legacy)
        self.assertEqual(result['schema_version'], 1)
        self.assertEqual(result['applies_to']['country'], legacy['country'])
        self.assertEqual(len(result['tracks']['focus']), 34)
        self.assertEqual(len(result['tracks']['research']), 48)
        priority = next(s for s in result['tracks']['focus'] if s.get('priority_when'))
        self.assertEqual(priority['priority_when']['op'], 'all')
        self.assertEqual(priority['action']['entity']['kind'], 'focus')

    def test_manual_checklists_are_isolated_even_when_step_keys_match(self):
        from hoi4companion.api import Store
        guides = {key: normalise_guide({'id': key, 'tracks': {'focus': [{'key': 'same', 'kind': 'gate'}]}})
                  for key in ('one', 'two')}
        store = Store.__new__(Store)
        store.lock = RLock(); store.guide_checks = {}; store.guide_revision = 0; store.default_guide_id = 'one'
        with patch('hoi4companion.api.load_guide', side_effect=lambda key=None: guides[key or 'one']):
            store.walkthrough_checks({'key': 'same', 'checked': True}, 'one')
            self.assertEqual(store.walkthrough_checks(guide_id='two')['checks'], {})
            store.walkthrough_checks({'key': 'same', 'checked': True}, 'two')
            store.walkthrough_checks({'reset': True}, 'one')
            self.assertEqual(store.walkthrough_checks(guide_id='two')['checks'], {'same': True})

    def test_declared_actions_require_consistent_entity_identity_and_unique_step_keys(self):
        for data in [
            {'schema_version': 2, 'tracks': {}},
            {'country': 'POL', 'applies_to': {'country': 'GER'}, 'tracks': {}},
            {'tracks': {'focus': [{'key': 'a', 'kind': 'gate'}, {'key': 'a', 'kind': 'gate'}]}},
        ]:
            with self.assertRaises(ValueError): normalise_guide(data)

    def test_canonical_state_unknown_validation_and_stale_removal(self):
        canonical = {'schema_version': 1, 'usable': True, 'facts': {
            'country': {'status': 'observed', 'value': 'AUS', 'source': 'process_memory'},
            'invalid': {'status': 'unknown', 'value': 12, 'source': ''}}}
        self.assertEqual(normalise_state(canonical)['facts']['invalid']['status'], 'unknown')
        self.assertIsNone(normalise_state(canonical)['facts']['invalid']['value'])
        canonical['usable'] = False
        self.assertFalse(normalise_state(canonical)['facts'])

    def test_shared_wire_examples_preserve_per_field_provenance(self):
        cases = json.loads((Path(__file__).parent / 'fixtures' / 'framework-state.json').read_text(encoding='utf-8'))
        for case in cases:
            result = normalise_state(case['input'])
            for key, expected in case['expected'].items():
                with self.subTest(key=key):
                    self.assertEqual({k: result['facts'][key][k] for k in expected}, expected)
            for key in case.get('absent', []):
                with self.subTest(absent=key):
                    self.assertNotIn(key, result['facts'])

    def test_unscoped_guide_resolves_unique_focus_and_requires_ambiguous_scope(self):
        from hoi4companion.guide import enrich_guide
        focus = SimpleNamespace(name='First', cost=10, prerequisites=[])
        raw = {'tracks': {'focus': [{'kind': 'focus', 'target': 'A'}]}}
        trees = {'one': SimpleNamespace(focuses={'A': focus})}
        step = enrich_guide(raw, trees, {}, {}, {})['tracks']['focus'][0]
        self.assertTrue(step['catalog_match'])
        self.assertEqual(step['entity']['schema_version'], 1)
        self.assertEqual(step['entity']['ref'], {'kind': 'focus', 'id': 'A', 'scope': 'one'})
        trees['two'] = trees['one']
        self.assertFalse(enrich_guide(raw, trees, {}, {}, {})['tracks']['focus'][0]['catalog_match'])

    def test_condition_truth_values_and_numeric_parity(self):
        state = {'schema_version': 1, 'usable': True, 'facts': {'values': fact([1], 'observed', 'fixture')}}
        self.assertTrue(evaluate_condition({'op': 'fact', 'key': 'values', 'test': 'contains', 'value': 1.0}, state))
        self.assertIsNone(evaluate_condition({'op': 'not', 'arg': {'op': 'check', 'key': 'missing'}}, state))
        for version in (None, True, 2):
            self.assertFalse(normalise_state({**state, 'schema_version': version})['usable'])
        self.assertFalse(normalise_state({**state, 'facts': None})['usable'])

    def test_live_state_does_not_load_optional_guide_content(self):
        from hoi4companion.api import Store
        import time
        store = Store.__new__(Store)
        store.lock = RLock(); store.settings = {'game_dir': ''}; store.live_provider_path = ''
        store.live_monitor = object(); store.live_cache_time = time.monotonic()
        store.live_cache = {'usable': False, 'sample': None}; store.default_guide_id = None
        with patch('hoi4companion.api.default_guide_id', side_effect=AssertionError('No content I/O on live path')):
            self.assertFalse(store.live_state()['usable'])


if __name__ == '__main__': unittest.main()
