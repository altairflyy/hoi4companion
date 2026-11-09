import io
import unittest

from test_word_guides import word
from hoi4companion.word_guides import prepare_document
from hoi4companion.guide_authoring import compile_document
from hoi4companion.guide_importer import preview


class ActionBlockTests(unittest.TestCase):
    def test_reading_glossary_uses_shared_vocabulary_without_guessing_ambiguous_codes(self):
        from hoi4companion.informal_guides import explanatory_terms
        rows = explanatory_terms('REL + NA AUS; Recruit CAV; SD; AE')
        self.assertEqual({r['code'] for r in rows}, {'REL', 'NA', 'CAV'})
        self.assertEqual(next(r['meaning'] for r in rows if r['code'] == 'NA'), 'non-aggression pact')
        self.assertEqual(explanatory_terms('MIL: Inf:ALL', 'production')[0]['domain'], 'production')
        self.assertEqual(explanatory_terms('as an ordinary sentence'), [])

    def test_explicit_then_uses_project_start_and_law_completion_without_guessing_other_prose(self):
        packet = prepare_document(io.BytesIO(word([[
            'Start special project: Test Project, then War Economy (150PP), then Limited Conscription (150PP)',
            'War Economy (150PP) & prepare the front'], [], []])))
        compiled = compile_document(packet['document'])
        result = preview(compiled['guide'], packet['sources'], [
            {'kind': 'project', 'id': 'project', 'name': 'Test Project'},
            {'kind': 'law', 'id': 'war', 'name': 'War Economy', 'group': 'economy'},
            {'kind': 'law', 'id': 'conscription', 'name': 'Limited Conscription', 'group': 'conscription'}])
        self.assertTrue(result['valid'], result['errors'])
        rows = result['guide']['tracks']['operations']
        self.assertEqual(len(rows), 4)
        self.assertEqual(rows[0]['action']['kind'], 'special_project')
        self.assertEqual(rows[1]['depends_on_started'], [rows[0]['key']])
        self.assertEqual(rows[2]['depends_on'], [rows[1]['key']])
        self.assertEqual(rows[3]['tracking'], 'reminder')
        facts = lambda **v: {'schema_version': 1, 'usable': True, 'facts': {
            k: {'status': 'observed', 'source': 'fixture', 'value': value} for k, value in v.items()}}
        from hoi4companion.guide_runtime import evaluate
        waiting = evaluate(result['guide'], facts(**{'projects.active': [], 'projects.completed': [], 'politics.ideas': []}))
        started = evaluate(result['guide'], facts(**{'projects.active': ['project'], 'projects.completed': [], 'politics.ideas': []}))
        self.assertEqual(waiting['steps'][1]['status'], 'waiting')
        self.assertEqual(started['steps'][1]['status'], 'ready')
        self.assertEqual(started['steps'][2]['status'], 'waiting')

    def test_source_action_numbers_survive_expansion_and_do_not_gate_parallel_columns(self):
        packet = prepare_document(io.BytesIO(word([
            ['Create Agency (spy pressure AUS)', 'War Economy (150PP)\n- prepare the front'],
            ['Basic Machine Tools'], ['First Focus']])))
        steps = packet['document']['guide']['steps']
        self.assertEqual([s['details']['source_action']['ordinal'] for s in steps[:2]], [1, 2])
        self.assertNotIn('details', steps[2])
        compiled = compile_document(packet['document'])
        result = preview(compiled['guide'], packet['sources'], [
            {'kind': 'law', 'id': 'war_economy', 'name': 'War Economy', 'group': 'economy'}])
        rows = result['guide']['tracks']['operations']
        agency = next(r for r in rows if r['action']['params'].get('operation_id') == 'create_intelligence_agency')
        self.assertIn('started_when', agency)
        self.assertIn('completion', agency)
        self.assertEqual(agency['action']['params']['source_action']['ordinal'], 1)
        pressure=next(r for r in rows if r['action']['params'].get('mission_id')=='diplomatic_pressure')
        self.assertNotEqual(pressure.get('tracking'),'reminder')
        self.assertEqual(pressure['action']['params']['target_country'],'AUS')
        self.assertIn('started_when',pressure)
        self.assertTrue(all(not r.get('depends_on') for r in rows))
        self.assertEqual([r['action']['params']['source_action']['ordinal'] for r in rows], [1, 1, 2, 2])

    def test_existing_word_import_recovers_blocks_without_rewriting_definitions(self):
        packet = prepare_document(io.BytesIO(word([['War Economy (150PP)\n- train', 'War Economy (150PP)'], [], []])))
        compiled = compile_document(packet['document'])
        for row in compiled['guide']['tracks']['operations']:
            row['action']['params'].pop('source_action', None)
        from copy import deepcopy
        before = deepcopy(compiled)
        report = preview(compiled['guide'], packet['sources'], [
            {'kind': 'law', 'id': 'war_economy', 'name': 'War Economy', 'group': 'economy'}])
        self.assertEqual(compiled, before)
        self.assertEqual([r['action']['params']['source_action']['ordinal'] for r in report['guide']['tracks']['operations']], [1, 1, 2])


if __name__ == '__main__':
    unittest.main()
