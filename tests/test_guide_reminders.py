import copy
import unittest

from hoi4companion.guide import manual_keys, normalise_guide, update_checks
from hoi4companion.guide_runtime import evaluate


def reminder(**extra):
    return {'key': 'method', 'action': {'kind': 'manual'}, 'tracking': 'reminder', **extra}


def definition(*rows):
    return {'id': 'reminders', 'tracks': {'focus': list(rows)}}


def focus():
    return {'key': 'focus', 'action': {'kind': 'select_focus', 'entity': {'kind': 'focus', 'id': 'focus'}}}


def live():
    return {'usable': True, 'sample': {'country': 'GER', 'completed_focuses': [], 'current_focus': None,
                                      'researched_technologies': [], 'research_slots': []}}


class ReminderTests(unittest.TestCase):
    def test_reminder_does_not_require_confirmation_or_block_next_action(self):
        guide = definition(reminder(), focus())
        before = copy.deepcopy(guide)
        progress = {'schema_version': 1, 'guide_id': 'reminders', 'run_id': 'r',
                    'checks': {'method': True}, 'started': {'method': True}}
        result = evaluate(guide, live(), progress)
        row = result['steps'][0]
        self.assertEqual((row['status'], row['reason'], row['completion_basis']),
                         ('ready', 'reminder_available', 'reminder'))
        self.assertEqual(row['tracking'], 'reminder')
        self.assertIsNone(row['completion']['value'])
        self.assertEqual(row['completion']['reason'], 'not_tracked')
        self.assertIsNone(row['selection']['value'])
        self.assertEqual(result['ready'], ['focus'])
        self.assertEqual(result['tracks']['focus']['next'], 'focus')
        self.assertEqual(result['parallel']['focus']['steps'], ['focus'])
        self.assertNotIn('method', manual_keys(normalise_guide(guide)))
        with self.assertRaises(ValueError):
            update_checks(normalise_guide(guide), {}, {'key': 'method', 'checked': True})
        self.assertEqual(guide, before)

    def test_unknown_reminder_visibility_does_not_hide_parallel_action(self):
        guide = definition(reminder(when={'op': 'fact', 'key': 'missing', 'test': 'eq', 'value': True}), focus())
        result = evaluate(guide, live())
        self.assertEqual(result['steps'][0]['reason'], 'condition_unknown')
        self.assertEqual(result['parallel']['focus']['steps'], ['focus'])

    def test_reminder_can_wait_for_real_action_without_becoming_completed(self):
        guide = definition(focus(), reminder(depends_on=['focus']))
        result = evaluate(guide, live())
        self.assertEqual(result['steps'][1]['status'], 'waiting')
        self.assertEqual(result['steps'][1]['blocked_by'], ['focus'])

    def test_non_completable_dependencies_and_conflicting_tracking_rejected(self):
        for change in ({'tracking': 'unknown'}, {'tracking': None},
                       {'completion': {'op': 'check', 'key': 'done'}},
                       {'started_when': {'op': 'check', 'key': 'started'}},
                       {'action': {'kind': 'free'}}, {'action': focus()['action']}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                normalise_guide(definition(reminder(**change)))
        with self.assertRaisesRegex(ValueError, 'promemoria'):
            normalise_guide(definition(reminder(), {**focus(), 'depends_on': ['method']}))

    def test_explicit_action_tracking_preserves_manual_confirmation(self):
        result = evaluate(definition({'key': 'manual', 'tracking': 'action', 'action': {'kind': 'manual'}}), live())
        self.assertEqual(result['steps'][0]['reason'], 'manual_confirmation_required')


if __name__ == '__main__':
    unittest.main()
