from copy import deepcopy
from unittest import TestCase
from hoi4companion.backend_core import _request
from hoi4companion.guide import normalise_guide
from test_research_upgrades import definition,tech

class NativeResearchImportTests(TestCase):
    def test_compilation_uses_the_catalogs_immutable_custom_vocabulary(self):
        from unittest.mock import patch
        from hoi4companion.research_upgrades import guide_language
        from hoi4companion.resolution import CatalogIndex
        vocabulary=deepcopy(guide_language());vocabulary['research_families']['fixture guns']='Artillery'
        entries=[tech('a',['b'],name='Artillery I'),tech('b',name='Artillery II')]
        with patch('hoi4companion.research_upgrades.guide_language',return_value=vocabulary):index=CatalogIndex(entries)
        guide,_=definition(['Fixture Guns UP']);guide=normalise_guide(guide)
        result=_request(0,'research_compile',guide=guide,**index.native_import_args())
        self.assertEqual(result['guide']['tracks']['research'][0]['relative_research']['chain'],['a','b'])

    def test_progressions_and_choices_match_independent_compiler(self):
        from hoi4companion._research_upgrades_reference import resolve_research_upgrades
        entries=[tech('a',['b'],name='Artillery I'),tech('b',name='Artillery II'),
                 tech('d1',['d2'],name='Dispersed Industry I'),tech('d2',name='Dispersed Industry II'),
                 tech('c1',['c2'],name='Concentrated Industry I'),tech('c2',name='Concentrated Industry II'),
                 tech('o1',name='Extraction I'),tech('o2',name='Extraction II')]
        for queries in (['Artillery UP','Artillery UP'],['Artillery I','Artillery UP'],
                        ['Concentrated/Dispersed Industry 1','Industry UP'],
                        ['Artillery UP/Extraction UP'],['Extraction UP','Extraction UP'],['unknown UP']):
            for context in (None,{'country':'TST','researched':['a'],'unknown':[],'date':'1936.1.1'}):
                guide,_=definition(queries,applies_to={'country':'TST'});guide=normalise_guide(guide)
                expected=deepcopy(guide);resolutions=resolve_research_upgrades(expected,entries,context)
                actual=_request(0,'research_compile',guide=guide,entries=entries,research_context=context)
                with self.subTest(queries=queries,context=context):
                    self.assertEqual(actual,{'guide':expected,'resolutions':resolutions})

    def test_next_useful_lane_and_ambiguity_preserve_source(self):
        entries=[tech('a',['b'],name='Artillery I'),tech('b',name='Artillery II')]
        guide,_=definition(['Artillery UP','Artillery UP']);guide=normalise_guide(guide);before=deepcopy(guide)
        result=_request(0,'research_compile',guide=guide,entries=entries)
        self.assertEqual([r['relative_research']['chain'] for r in result['guide']['tracks']['research']],[['a','b'],['a','b']])
        self.assertEqual(guide,before)
        ambiguous=[*entries,tech('c',name='Artillery I')]
        other=_request(0,'research_compile',guide=guide,entries=ambiguous)
        self.assertEqual(other['resolutions']['r0']['reason'],'upgrade_branch_ambiguous')

    def test_country_history_does_not_override_declared_guide_country(self):
        guide,_=definition(['Artillery UP']);guide=normalise_guide(guide);guide['applies_to']['country']='TST'
        with self.assertRaisesRegex(ValueError,'nazione diversa'):
            _request(0,'research_compile',guide=guide,entries=[tech('a',name='Artillery I')],research_context={'country':'OTHER','researched':[]})
