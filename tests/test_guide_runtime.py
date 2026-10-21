import copy
import json
from pathlib import Path
import unittest

from hoi4companion.guide_runtime import evaluate


def observed(value):
    return {'status': 'observed', 'value': value, 'source': 'fixture'}


def state(**facts):
    result = {'country': observed('GER'), 'focus.tree': observed('tree'),
              'focus.completed': observed([]), 'focus.active': observed([]),
              'research.completed': observed([]), 'research.active': observed([]),
              'research.slots': observed([{'technology_id': None}, {'technology_id': None}])}
    result.update({key: observed(value) for key, value in facts.items()})
    return {'schema_version': 1, 'usable': True, 'session_id': 'session', 'facts': result}


def focus(key, **extra):
    return {'key': key, 'action': {'kind': 'select_focus', 'entity': {'kind': 'focus', 'id': key}}, **extra}


def guide(*rows):
    return {'schema_version': 1, 'id': 'generic', 'tracks': {'focus': list(rows)}}


class GuideRuntimeTests(unittest.TestCase):
    def test_shared_runtime_fixtures_are_pure_and_deterministic(self):
        cases = json.loads((Path(__file__).parent / 'fixtures' / 'guide-runtime.json').read_text(encoding='utf-8'))
        for example in cases:
            with self.subTest(example=example['name']):
                before = copy.deepcopy(example)
                result = evaluate(example['guide'], example['state'], example.get('progress'))
                expected = example['expected']
                self.assertEqual([row['status'] for row in result['steps']], expected['statuses'])
                if 'keys' in expected:
                    self.assertEqual([row['key'] for row in result['steps']], expected['keys'])
                for field in ('mode', 'progress_status'):
                    if field in expected:
                        self.assertEqual(result[field], expected[field])
                for lane, keys in expected.get('parallel', {}).items():
                    self.assertEqual(result['parallel'][lane]['steps'], keys)
                self.assertEqual(example, before)
                self.assertEqual(evaluate(example['guide'], example['state'], example.get('progress')), result)

    def test_dependency_order_and_recalculation_are_pure(self):
        definition = guide(focus('last', depends_on=['first']), focus('first'))
        live = state()
        before = copy.deepcopy((definition, live))
        result = evaluate(definition, live)
        self.assertEqual([s['status'] for s in result['steps']], ['waiting', 'ready'])
        self.assertEqual(result['steps'][0]['blocked_by'], ['first'])
        self.assertEqual(result['parallel']['focus']['steps'], ['first'])
        complete = evaluate(definition, state(**{'focus.completed': ['first']}))
        self.assertEqual([s['status'] for s in complete['steps']], ['ready', 'completed'])
        self.assertEqual(complete['parallel']['focus']['steps'], ['last'])
        self.assertEqual(before, (definition, live))

    def test_unknown_is_not_completion_or_dependency_success(self):
        live = state()
        del live['facts']['focus.completed']
        result = evaluate(guide(focus('a'), focus('b', depends_on=['a'])), live)
        self.assertEqual([s['status'] for s in result['steps']], ['unverifiable', 'unverifiable'])
        self.assertEqual(result['steps'][1]['reason'], 'dependency_unknown')
        self.assertEqual(result['steps'][0]['completion']['status'], 'unknown')

    def test_progress_is_bound_to_guide_and_session(self):
        definition = guide({'key': 'gate', 'action': {'kind': 'manual'}}, focus('a'))
        progress = {'schema_version': 1, 'guide_id': 'generic', 'run_id': 'run',
                    'session_id': 'session', 'checks': {'gate': True, 'a': True}}
        accepted = evaluate(definition, state(), progress)
        self.assertEqual(accepted['progress_status'], 'accepted')
        self.assertEqual([s['status'] for s in accepted['steps']], ['completed', 'ready'])
        wrong = evaluate(definition, state(), {**progress, 'session_id': 'old'})
        self.assertEqual(wrong['progress_status'], 'wrong_session')
        self.assertEqual(wrong['steps'][1]['status'], 'unverifiable')
        self.assertIsNone(wrong['run_id'])

    def test_completed_and_active_dominate_conditions_and_dependencies(self):
        definition = guide(focus('done', when={'op': 'check', 'key': 'blocked'}),
                           focus('active', depends_on=['done'], skip_when={'op': 'check', 'key': 'skip'}))
        result = evaluate(definition, state(**{'focus.completed': ['done'], 'focus.active': ['active']}))
        self.assertEqual([s['status'] for s in result['steps']], ['completed', 'active'])
        self.assertEqual(result['parallel']['focus']['capacity'], 0)

    def test_explicit_completion_and_conditional_priority(self):
        definition = guide(focus('first'), focus('second', priority=10,
            priority_when={'op': 'check', 'key': 'boost'},
            completion={'op': 'check', 'key': 'done'}))
        progress = {'schema_version': 1, 'guide_id': 'generic', 'run_id': 'r', 'checks': {'boost': True, 'done': False}}
        result = evaluate(definition, state(), progress)
        self.assertEqual(result['tracks']['focus']['order'], ['second', 'first'])
        self.assertEqual(result['steps'][1]['completion_basis'], 'condition')
        self.assertEqual(result['steps'][1]['status'], 'ready')

    def test_invalid_progress_and_cycles_are_rejected(self):
        with self.assertRaises(ValueError):
            evaluate(guide(focus('a')), state(), {'schema_version': 1, 'guide_id': 'generic', 'run_id': ''})
        with self.assertRaises(ValueError):
            evaluate(guide(focus('a', depends_on=['b']), focus('b', depends_on=['a'])), state())

    def test_six_statuses_and_not_applicable_dependency(self):
        definition = guide(focus('done'), focus('active'), focus('ready'),
                           focus('waiting', when={'op': 'check', 'key': 'gate'}),
                           focus('skip', skip_when={'op': 'check', 'key': 'skip'}),
                           focus('unknown', completion={'op': 'fact', 'key': 'absent', 'test': 'eq', 'value': True}),
                           focus('after_skip', depends_on=['skip']))
        progress = {'schema_version': 1, 'guide_id': 'generic', 'run_id': 'r', 'checks': {'gate': False, 'skip': True}}
        result = evaluate(definition, state(**{'focus.completed': ['done'], 'focus.active': ['active']}), progress)
        self.assertEqual([row['status'] for row in result['steps']],
                         ['completed', 'active', 'ready', 'waiting', 'not_applicable', 'unverifiable', 'waiting'])
        self.assertEqual(result['steps'][-1]['blocked_by'], ['skip'])

    def test_manual_barrier_is_opted_out_only_by_explicit_dependencies(self):
        definition = guide({'key': 'gate', 'action': {'kind': 'manual'}}, focus('blocked'),
                           focus('independent', depends_on=[]))
        result = evaluate(definition, state())
        self.assertEqual(result['steps'][1]['reason'], 'dependency_unknown')
        self.assertEqual(result['steps'][2]['status'], 'ready')
        progress = {'schema_version': 1, 'guide_id': 'generic', 'run_id': 'r', 'checks': {'gate': False}}
        self.assertEqual(evaluate(definition, state(), progress)['steps'][1]['status'], 'waiting')

    def test_project_does_not_block_parallel_research(self):
        definition = {'id': 'generic', 'tracks': {'research': [
            {'key': 'project', 'action': {'kind': 'special_project', 'entity': {'kind': 'project', 'id': 'p'}}},
            {'key': 'a', 'action': {'kind': 'research', 'entity': {'kind': 'technology', 'id': 'a'}}},
            {'key': 'b', 'action': {'kind': 'research', 'entity': {'kind': 'technology', 'id': 'b'}}},
            {'key': 'c', 'action': {'kind': 'research', 'entity': {'kind': 'technology', 'id': 'c'}}}]}}
        result = evaluate(definition, state())
        self.assertEqual(result['parallel']['research'], {'capacity': 2, 'steps': ['a', 'b']})
        live = state()
        del live['facts']['research.slots']
        unknown = evaluate(definition, live)
        self.assertEqual(unknown['parallel']['research'], {'capacity': None, 'steps': []})
        self.assertIn('a', unknown['ready'])

    def test_scope_mismatch_and_unknown_identity_are_distinct(self):
        definition = {**guide(focus('a')), 'applies_to': {'country': 'GER', 'tree_id': 'tree'}}
        wrong = evaluate(definition, state(country='POL', **{'focus.completed': ['a']}))
        self.assertEqual(wrong['mode'], 'wrong_country')
        self.assertEqual(wrong['steps'][0]['status'], 'not_applicable')
        self.assertEqual(wrong['steps'][0]['completion']['status'], 'unknown')
        live = state(**{'focus.completed': ['a']})
        del live['facts']['country']
        missing = evaluate(definition, live)
        self.assertEqual(missing['mode'], 'preview')
        self.assertEqual(missing['steps'][0]['status'], 'unverifiable')
        self.assertEqual(missing['steps'][0]['completion']['status'], 'unknown')

    def test_phase_conditions_and_manual_completion_in_preview(self):
        definition = guide({'key': 'manual', 'action': {'kind': 'manual'}}, focus('a', phase='early'))
        definition['phases'] = [{'id': 'early', 'title': 'Early', 'when': {'op': 'check', 'key': 'phase'}}]
        progress = {'schema_version': 1, 'guide_id': 'generic', 'run_id': 'r', 'checks': {'phase': False, 'manual': True}}
        self.assertEqual(evaluate(definition, state(), progress)['steps'][1]['reason'], 'phase_condition_false')
        preview = evaluate(definition, None, progress)
        self.assertEqual(preview['steps'][0]['status'], 'completed')
        self.assertEqual(preview['steps'][1]['status'], 'unverifiable')

    def test_progress_validation_and_binding_never_use_unknown_native_completion(self):
        definition = guide(focus('a'))
        progress = {'schema_version': 1, 'guide_id': 'generic', 'run_id': 'r'}
        self.assertEqual(evaluate(definition, state(), progress)['progress_status'], 'accepted')
        for change in ({'schema_version': True}, {'run_id': ' '}, {'checks': {'a': 1}}, {'session_id': ''}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                evaluate(definition, state(), {**progress, **change})
        self.assertEqual(evaluate(definition, state(), {**progress, 'guide_id': 'other'})['progress_status'], 'wrong_guide')
        self.assertEqual(evaluate(definition, None, {**progress, 'session_id': 'session'})['progress_status'], 'session_unverified')
        live = state()
        del live['facts']['focus.completed']
        result = evaluate(definition, live, {**progress, 'checks': {'a': True}})
        self.assertEqual(result['steps'][0]['completion']['status'], 'unknown')

    def test_deep_dependencies_are_iterative(self):
        rows = [focus(str(index), depends_on=[str(index - 1)] if index else []) for index in range(2000)]
        completed = [str(index) for index in range(1999)]
        result = evaluate(guide(*rows), state(**{'focus.completed': completed}))
        self.assertEqual(result['steps'][-1]['status'], 'ready')
        self.assertEqual(result['parallel']['focus']['steps'], ['1999'])


if __name__ == '__main__':
    unittest.main()
