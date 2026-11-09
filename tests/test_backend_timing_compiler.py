from copy import deepcopy
from unittest import TestCase
from hoi4companion.backend_core import _request
from hoi4companion._timing_conditions_reference import compile_timing
from hoi4companion._action_blocks_reference import annotate
import test_timing_compiler as fixtures


class NativeTimingCompilerTests(TestCase):
    def test_list_catalog_owner_remains_alive_through_the_native_request(self):
        from hoi4companion.timing_conditions import compile_timing as native
        from hoi4companion.event_instructions import event_command
        from hoi4companion.informal_targets import named_command
        from hoi4companion.action_commands import sequence
        self.assertEqual(native('At 200PP',[]),{'op':'fact','key':'country.politics.political_power','test':'gte','value':200})
        self.assertIsNone(event_command('Event: Missing; choose: Absent',[],'TST'))
        self.assertIsNone(named_command('Absent',[],'TST'))
        self.assertIsNone(sequence('Absent then Missing',[],'TST'))
    def test_full_atoms_compounds_unknowns_and_scoped_identities_match(self):
        phrases = ['At 200PP', 'At 12.5 army experience', 'Once 20 NE', 'When 50 AFE',
                   'At 60AE', 'After the war', 'Dopo la guerra',
                   'On “Institute Price Controls”', 'On Intervention in Spain focus',
                   'On “Institute Price Controls” done', 'After Institute Price Controls',
                   'Before picking Institute Price Controls', 'After Basic Machine Tools research',
                   'After Eng3 research', 'At 200PP and after Basic Machine Tools research',
                   'At 200PP or after Basic Machine Tools research',
                   'At 200PP and when ready', 'At 200PP, invasion ready',
                   'When ready or after Basic Machine Tools research', 'At 20% paranoia',
                   'On Claim', 'After landing in UK', 'After the war with France and at 200PP',
                   'On GER down / no later than April 39', 'No later than January 1940',
                   'At 200PP and At 50AE or At 10NE', 'At 200PP; At 50AE',
                   'On "Intervention in Spain" or At 200PP', None, '', 'x'*1001]
        for country, scope, currency in (('TST','tree',None),('ALT',None,'air_experience')):
            for phrase in phrases:
                with self.subTest(phrase=phrase, country=country):
                    expected=compile_timing(phrase,fixtures.CATALOG,country,scope=scope,currency=currency)
                    actual=_request(0,'import_timing',text=phrase,country=country,scope=scope,currency=currency,
                                    **fixtures.CATALOG.native_import_args())
                    self.assertEqual(actual,expected)
        self.assertEqual(_request(0,'import_timing',text='At 200PP'),compile_timing('At 200PP'))

    def test_source_action_blocks_preserve_ordinals_gaps_and_explicit_metadata(self):
        guide={'tracks':{'operations':[
            {'key':'word-0010--operation-2','action':{'kind':'manual'},'evidence':[{'source_id':'s','quote':'Appoint expert'}]},
            {'key':'word-0002','action':{'kind':'manual'},'evidence':[{'source_id':'s','quote':'Build factories'}]},
            {'key':'word-0011','action':{'kind':'research'},'evidence':[{'source_id':'s','quote':'Tools'}]},
        ]},'sources':[{'id':'s','type':'file','text':'[Column 1]\nBuild factories\n[Column 2]\nTools\n[Column 1]\nAppoint expert\n[Column 3]\nFocus'}]}
        expected=deepcopy(guide);changes=annotate(expected)
        self.assertEqual(_request(0,'import_action_blocks',guide=guide),{'guide':expected,'changes':changes})
        self.assertEqual(_request(0,'import_action_blocks',guide=expected),{'guide':expected,'changes':[]})
