import unittest
from hoi4companion.clausewitz import parse
from hoi4companion.script_analysis import RuleBook, analyze_documents


def documents(focus, extra=()):
    return [('common/national_focus/test.txt',parse('focus_tree={id=t '+focus+'}')),*extra]


class ScriptAnalysisTests(unittest.TestCase):
    def codes(self,report):return {row['code'] for row in report['diagnostics']}

    def test_real_cwtools_rules_are_pinned_and_describe_parameters(self):
        book=RuleBook.load()
        info=book.describe('trigger','has_completed_focus')
        self.assertEqual(book.provenance['revision'],'297dd69c42aabe10078a8b7ab7f2d07ee631d4ef')
        self.assertTrue(info['variants'])
        self.assertIn('country',info['scopes'])
        self.assertIn('<focus>',str(info))

    def test_references_include_shared_focus_and_missing_ids_are_diagnosed(self):
        report=analyze_documents(documents('focus={id=A prerequisite={focus=S} available={has_completed_focus=MISSING}} shared_focus={id=S}'))
        missing=[d for d in report['diagnostics'] if d['code']=='reference_missing']
        self.assertEqual([d['value'] for d in missing],['MISSING'])
        self.assertTrue(all(d['source']=='static_analysis' for d in report['diagnostics']))

    def test_scope_transitions_and_unknown_context_do_not_invent_errors(self):
        report=analyze_documents(documents('focus={id=A available={resistance>2 capital_scope={resistance>2} FROM={resistance>2}}}'))
        errors=[d for d in report['diagnostics'] if d['code']=='scope_mismatch']
        self.assertEqual(len(errors),1)
        self.assertIn('resistance',errors[0]['path'])
        self.assertGreater(report['coverage']['unknown_scope'],0)

    def test_parameter_types_and_overloads_are_checked_without_evaluating_effects(self):
        report=analyze_documents(documents('focus={id=A available={always=perhaps} completion_reward={add_stability=@dynamic set_capital={state=1}}}',
            [('history/states/one.txt',parse('state={id=1}'))]))
        self.assertIn('parameter_type',self.codes(report))
        self.assertFalse(any(d['value']=='@dynamic' for d in report['diagnostics']))
        self.assertNotIn('completed',report)

    def test_localisation_missing_nested_refs_cycles_and_translation_fallback(self):
        report=analyze_documents(documents('focus={id=A}'),labels={'A':'$loop$','loop':'$A$'},reference_labels={'A':'English','A_desc':'$missing_key$'})
        self.assertIn('localisation_cycle',self.codes(report))
        self.assertIn('localisation_reference_missing',self.codes(report))
        self.assertIn('localisation_fallback',self.codes(report))

    def test_unknown_mod_scripts_and_dynamic_targets_remain_unverifiable(self):
        report=analyze_documents(documents('focus={id=A available={mod_unknown={has_completed_focus=MISSING} has_completed_focus=[ROOT.GetFocus]}}'))
        self.assertNotIn('reference_missing',self.codes(report))
        self.assertGreater(report['coverage']['unsupported'],0)
        self.assertEqual(report['status'],'partial')

    def test_incomplete_namespace_never_proves_a_reference_is_missing(self):
        report=analyze_documents(documents('focus={id=A prerequisite={focus=MISSING}}'),incomplete={'focus'})
        self.assertNotIn('reference_missing',self.codes(report))
        self.assertGreater(report['coverage']['unsupported'],0)

    def test_diagnostic_limit_keeps_total_counts_and_signals_truncation(self):
        report=analyze_documents(documents('focus={id=A available={always=wrong always=wrong always=wrong}}'),limit=1)
        self.assertEqual(len(report['diagnostics']),1)
        self.assertGreater(report['summary']['total'],1)
        self.assertTrue(report['truncated'])

    def test_unknown_parameter_and_chained_scope_values_are_advisory(self):
        report=analyze_documents(documents('focus={id=A available={is_ally_with=ROOT.owner} completion_reward={set_capital={state=1 unknown_field=yes}}}',
            [('history/states/one.txt',parse('state={id=1}'))]))
        self.assertNotIn('reference_missing',self.codes(report))
        self.assertGreater(report['coverage']['unhandled']['parameter:unknown_field'],0)
        self.assertTrue(all(d['severity']=='warning' for d in report['diagnostics']))

    def test_cwtools_scope_fields_are_blocks_and_country_aliases_are_valid_references(self):
        report=analyze_documents(documents('focus={id=A completion_reward={random_owned_state={add_manpower=100} give_military_access=SOU}}',
            [('common/country_tag_aliases/test.txt',parse('SOU={original_tag=SOV}'))]))
        self.assertNotIn('reference_missing',self.codes(report))
        self.assertNotIn('parameter_type',self.codes(report))

    def test_numeric_and_country_scoped_expressions_are_not_literal_missing_tags(self):
        report=analyze_documents(documents('focus={id=A available={is_ally_with=66.owner is_ally_with=DEN.hosts_danish_brigades}}',[
            ('history/states/silesia.txt',parse('state={id=66}')),
            ('common/country_tags/tags.txt',parse('DEN="countries/Denmark.txt"'))]))
        self.assertNotIn('reference_missing',self.codes(report))
        self.assertGreater(report['coverage']['unsupported'],0)

    def test_literal_country_tag_missing_from_a_complete_index_is_still_reported(self):
        report=analyze_documents(documents('focus={id=A available={is_ally_with=ZZZ}}',[
            ('common/country_tags/tags.txt',parse('DEN="countries/Denmark.txt"'))]))
        self.assertIn('reference_missing',self.codes(report))

    def test_block_overload_reports_cwt_range_conflict_instead_of_wrong_scalar_type(self):
        report=analyze_documents(documents('focus={id=A completion_reward={country_event={id=test.1 days=90 random_days=30}}}',
            [('events/test.txt',parse('country_event={id=test.1}'))]))
        self.assertNotIn('parameter_type',self.codes(report))
        self.assertIn('parameter_range',self.codes(report))
        self.assertNotIn('reference_missing',self.codes(report))

    def test_event_subtype_reference_still_reports_a_genuinely_missing_id(self):
        report=analyze_documents(documents('focus={id=A completion_reward={country_event={id=MISSING days=1}}}',
            [('events/test.txt',parse('country_event={id=test.1}'))]))
        self.assertIn('reference_missing',self.codes(report))

    def test_focus_text_override_is_the_localisation_key_not_the_focus_id(self):
        report=analyze_documents(documents('focus={id=A text=generic_focus}'),labels={'generic_focus':'A focus'},reference_labels={'generic_focus':'A focus'})
        self.assertNotIn('localisation_missing',self.codes(report))
