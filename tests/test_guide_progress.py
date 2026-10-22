import itertools
import unittest

from hoi4companion.guide_progress import RunStore


class GuideProgressTests(unittest.TestCase):
    def setUp(self):
        sequence = itertools.count(1)
        self.store = RunStore(id_factory=lambda: 'run-' + str(next(sequence)))

    def test_ensure_imports_legacy_once_and_preserves_false(self):
        first = self.store.ensure('one', {'done': True, 'pending': False, 'invalid': 1})
        self.assertEqual(first, {'schema_version': 1, 'guide_id': 'one', 'run_id': 'run-1',
                                 'session_id': None, 'checks': {'done': True, 'pending': False}, 'started': {}})
        self.assertNotIn('unknown', first['checks'])
        self.assertEqual(self.store.ensure('one', {'replacement': True}), first)

    def test_guides_and_runs_are_isolated_and_historical_runs_can_be_selected(self):
        first = self.store.ensure('one', {'done': True})
        other = self.store.ensure('two', {'other': True})
        fresh = self.store.create('one', session_id='session')
        self.assertEqual(fresh['checks'], {})
        self.assertEqual(self.store.get('one', first['run_id'])['checks'], {'done': True})
        self.assertEqual(self.store.get('two'), other)
        self.assertEqual(self.store.select('one', first['run_id']), first)
        self.assertEqual(self.store.get('one'), first)
        with self.assertRaises(ValueError):
            self.store.select('two', first['run_id'])
        with self.assertRaises(ValueError):
            self.store.get('missing')

    def test_snapshots_and_metadata_never_expose_mutable_internal_checks(self):
        snapshot = self.store.ensure('one', {'done': True})
        snapshot['checks']['done'] = False
        snapshot['session_id'] = 'tampered'
        self.assertEqual(self.store.get('one')['checks'], {'done': True})
        changed = self.store.update('one', {'done': False})
        changed['checks']['extra'] = True
        self.assertEqual(self.store.get('one')['checks'], {'done': False})
        metadata = self.store.metadata()
        self.assertEqual(metadata['one'], {'schema_version': 1, 'guide_id': 'one', 'run_id': 'run-1', 'session_id': None})
        metadata['one']['run_id'] = 'tampered'
        self.assertEqual(self.store.metadata()['one']['run_id'], 'run-1')

    def test_binding_none_same_and_changed_sessions_preserves_history(self):
        first = self.store.ensure('one', {'done': True})
        self.store.ensure('two', {'other': True})
        self.assertFalse(self.store.bind(None))
        self.assertTrue(self.store.bind('session-1'))
        self.assertFalse(self.store.bind('session-1'))
        self.assertEqual(self.store.get('one')['run_id'], first['run_id'])
        self.assertTrue(self.store.bind('session-2'))
        self.assertEqual(self.store.get('one')['checks'], {})
        self.assertEqual(self.store.get('two')['checks'], {})
        previous = self.store.get('one', first['run_id'])
        self.assertEqual(previous['session_id'], 'session-1')
        self.assertEqual(previous['checks'], {'done': True})

    def test_updates_validate_before_mutating_and_replace_the_current_checks(self):
        self.store.ensure('one', {'a': True})
        for invalid in (None, [], {'a': 1}, {1: True}, {'a': None}):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                self.store.update('one', invalid)
            self.assertEqual(self.store.get('one')['checks'], {'a': True})
        self.assertEqual(self.store.update('one', {'b': False})['checks'], {'b': False})

    def test_started_is_independent_preserved_by_old_writes_and_isolated_per_run(self):
        first = self.store.ensure('one')
        self.store.update('one', {}, started={'build': True})
        self.assertEqual(self.store.update('one', {'done': False})['started'], {'build': True})
        self.assertNotIn('started', self.store.metadata()['one'])
        with self.assertRaises(ValueError):
            self.store.update('one', {'done': True}, started={'build': 'yes'})
        self.assertEqual(self.store.get('one')['checks'], {'done': False})
        self.store.create('one')
        self.assertEqual(self.store.get('one')['started'], {})
        self.assertEqual(self.store.get('one', first['run_id'])['started'], {'build': True})

    def test_id_collisions_do_not_overwrite_existing_runs(self):
        store = RunStore(id_factory=lambda: 'same')
        first = store.ensure('one', {'done': True})
        with self.assertRaises(ValueError):
            store.create('one')
        self.assertEqual(store.get('one'), first)
        with self.assertRaises(ValueError):
            store.ensure('two')
        self.assertEqual(list(store.metadata()), ['one'])

    def test_session_rotation_is_atomic_when_a_later_id_collides(self):
        identifiers = iter(['first', 'second', 'replacement', 'replacement'])
        store = RunStore(id_factory=lambda: next(identifiers))
        store.ensure('one', {'done': True})
        store.ensure('two', {'done': False})
        store.bind('session-1')
        before = store.metadata()
        with self.assertRaises(ValueError):
            store.bind('session-2')
        self.assertEqual(store.metadata(), before)
        self.assertEqual(store.get('one')['checks'], {'done': True})
        self.assertEqual(store.get('two')['checks'], {'done': False})
        with self.assertRaises(ValueError):
            store.get('one', 'replacement')

    def test_identifiers_and_sessions_are_validated_before_store_mutation(self):
        for identifier in (None, 1, '', ' ', 'a' * 161):
            store = RunStore(id_factory=lambda identifier=identifier: identifier)
            with self.subTest(identifier=identifier), self.assertRaises(ValueError):
                store.create('one')
            self.assertEqual(store.metadata(), {})
        self.store.ensure('one')
        for session in ('', ' ', False, 1):
            with self.subTest(session=session), self.assertRaises(ValueError):
                self.store.bind(session)
            with self.subTest(create_session=session), self.assertRaises(ValueError):
                self.store.create('one', session_id=session)
        self.assertIsNone(self.store.get('one')['session_id'])

    def test_default_factory_creates_distinct_uuid_identifiers(self):
        store = RunStore()
        first = store.ensure('one')
        second = store.create('one')
        self.assertRegex(first['run_id'], r'^[0-9a-f]{32}$')
        self.assertNotEqual(first['run_id'], second['run_id'])


if __name__ == '__main__':
    unittest.main()
