"""Curated operational data and old checklist compatibility, no game access."""
import unittest
from hoi4companion.guide import load_guide, manual_keys
from hoi4companion.guide_runtime import evaluate


class OperationsTests(unittest.TestCase):
    def test_operations_are_declarative_and_native_tracks_are_preserved(self):
        guide = load_guide()
        self.assertEqual(len(guide['tracks']['focus']), 34)
        self.assertEqual(len(guide['tracks']['research']), 48)
        operations = guide['tracks']['operations']
        self.assertTrue(operations)
        self.assertTrue(all(row['action']['kind'] == 'manual' for row in operations))
        self.assertTrue(all(row['depends_on'] == [] for row in operations))
        self.assertTrue(all(row['evidence'][0]['source_id'] == 'video' for row in operations))
        self.assertTrue(all(row['action']['params']['timing'] for row in operations))
        categories = {row['action']['params']['category'] for row in operations}
        self.assertTrue({'construction', 'politics', 'production', 'army', 'characters', 'templates'} <= categories)

    def test_new_construction_check_does_not_confirm_agency_or_training(self):
        guide = load_guide()
        result = evaluate(guide, None, {'schema_version': 1, 'guide_id': guide['id'], 'run_id': 'r',
                                        'checks': {'setup_construction': True}})
        rows = {row['key']: row for row in result['steps']}
        self.assertEqual(rows['setup_construction']['status'], 'completed')
        for key in ['setup_agency', 'setup_mio', 'setup_training']:
            self.assertNotEqual(rows[key]['status'], 'completed')
        self.assertIn('setup_construction', manual_keys(guide))

    def test_old_combined_setup_confirmation_is_accepted_but_cannot_override_agency_observation(self):
        guide = load_guide()
        result = evaluate(guide, None, {'schema_version': 1, 'guide_id': guide['id'], 'run_id': 'old',
                                        'checks': {'m_setup': True}})
        rows = {row['key']: row for row in result['steps']}
        for key in ['setup_construction', 'setup_mio', 'setup_training']:
            self.assertEqual(rows[key]['status'], 'completed')
        self.assertEqual(rows['setup_agency']['status'], 'unverifiable')
        self.assertTrue({'m_setup', 'setup_agency'} <= manual_keys(guide))

    def test_agency_creation_uses_qualified_live_flags_not_manual_overrides(self):
        guide = load_guide()
        progress = {'schema_version': 1, 'guide_id': guide['id'], 'run_id': 'r',
                    'checks': {'m_setup': True, 'setup_agency': True}, 'started': {'setup_agency': True}}
        for created, creating, expected in ((None, None, 'unverifiable'), (False, False, 'ready'),
                                            (False, True, 'active'), (True, False, 'completed'),
                                            (True, True, 'completed')):
            with self.subTest(created=created, creating=creating):
                intelligence = {'status': 'observed', 'is_created': created, 'in_creation': creating}
                state = {'usable': True, 'sample': {'country': 'POL', 'focus_tree_id': 'polish_focus',
                         'country_overview': {'country': 'POL', 'intelligence': intelligence}}}
                row = next(row for row in evaluate(guide, state, progress)['steps'] if row['key'] == 'setup_agency')
                self.assertEqual(row['status'], expected)
                self.assertEqual(row['completion']['value'], created)
                self.assertEqual(row['completion_basis'], 'condition')
                if expected == 'active':
                    self.assertEqual(row['selection']['source'], 'declared_start')

    def test_agency_unknown_stale_and_other_country_cannot_complete(self):
        guide = load_guide()
        for status, usable, country in (('unavailable', True, 'POL'), ('partial', True, 'POL'),
                                        ('observed', False, 'POL'), ('observed', True, 'ROM')):
            with self.subTest(status=status, usable=usable, country=country):
                # Unqualified native sections publish no decoded flag values.
                intelligence = {'status': status}
                if status == 'observed':
                    intelligence.update(is_created=True, in_creation=True)
                state = {'usable': usable, 'sample': {'country': country, 'focus_tree_id': 'polish_focus',
                         'country_overview': {'country': country, 'intelligence': intelligence}}}
                progress = {'schema_version': 1, 'guide_id': guide['id'], 'run_id': 'r',
                            'checks': {'setup_agency': True, 'm_setup': True}, 'started': {'setup_agency': True}}
                row = next(row for row in evaluate(guide, state, progress)['steps'] if row['key'] == 'setup_agency')
                self.assertNotIn(row['status'], ('active', 'completed'))
                self.assertIsNone(row['completion']['value'])

    def test_construction_starts_from_declared_queue_without_manual_check(self):
        guide = load_guide()
        state = {'schema_version': 1, 'usable': True, 'session_id': 's', 'facts': {
            key: {'status': 'observed', 'value': value, 'source': 'fixture'}
            for key, value in {'country': 'POL', 'focus.tree': 'polish_focus',
                               'construction.queued': ['state:762:arms_factory']}.items()}}
        progress = {'schema_version': 1, 'guide_id': guide['id'], 'run_id': 'r', 'checks': {}}
        def construction(value):
            return next(row for row in evaluate(guide, value, progress)['steps'] if row['key'] == 'setup_construction')
        self.assertEqual(construction(state)['status'], 'active')
        self.assertNotEqual(construction(state)['completion']['value'], True)
        state['facts']['construction.queued']['value'] = []
        self.assertNotIn(construction(state)['status'], ('active', 'completed'))
        state['facts']['construction.queued']['value'] = ['state:10:arms_factory']
        self.assertNotEqual(construction(state)['status'], 'active')
        self.assertNotEqual(construction({'usable': False})['status'], 'active')


if __name__ == '__main__':
    unittest.main()
