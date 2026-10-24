"""Integration tests against the actual packaged CWTools host and Rust ABI.

Build both native components before running; an absent host is a deployment
failure, not a reason to substitute the old partial analyzer or skip silently.
"""
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from hoi4companion.cwtools_engine import analyze


class CwtoolsHostTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp=TemporaryDirectory();root=Path(cls.temp.name)
        cls.addClassCleanup(cls.temp.cleanup)
        def source(relative,text):
            file=root/relative;file.parent.mkdir(parents=True,exist_ok=True)
            file.write_text(text,encoding='utf-8-sig');return str(file)
        cls.files={
            'common/national_focus/test.txt':source('focus.txt','''
                shared_focus={id=SHARED icon=GFX_test x=0 y=0 cost=10}
                focus_tree={id=T country={factor=1} focus={
                    id=A icon=GFX_test x=0 y=0 cost=10
                    prerequisite={focus=SHARED}
                    available={always=perhaps resistance > 1}
                    completion_reward={country_event={id=test.1 days=1}}
                } focus={id=B_FOCUS icon=GFX_test x=1 y=1 cost=10 prerequisite={focus=MISSING}}
                }
            '''),
            'events/test.txt':source('event.txt','''add_namespace=test
                country_event={id=test.1 title=TITLE_MISSING desc=DESC is_triggered_only=yes
                    option={name=OPTION add_stability=0.1}}
            '''),
            'interface/test.gfx':source('interface.gfx','spriteTypes={spriteType={name=GFX_test texturefile="gfx/test.dds"}}'),
            'gfx/test.dds':source('empty.dds',''),
            'localisation/test_l_english.yml':source('test_l_english.yml','l_english:\n T:0 "Tree"\n A:0 "Focus"\n SHARED:0 "Shared"\n OPTION:0 "Option"\n TITLE:0 "Title"\n DESC:0 "[This.CompletelyInvalidCommand]"\n'),
        }
        cls.result=analyze(cls.files)

    def test_upstream_aliases_cardinality_references_and_scope_validation(self):
        result=self.result;self.assertEqual(result['engine'],'cwtools')
        self.assertEqual(result['parser_errors'],[])
        rows=result['diagnostics'];messages='\n'.join(row['message'] for row in rows)
        self.assertIn('Expecting yes or no, got perhaps',messages)
        self.assertTrue(any(row['code']=='CW247' for row in rows),messages)
        missing=[row for row in rows if row['category']=='references']
        self.assertTrue(any('type focus' in row['message'] for row in missing),messages)
        # The typed shared-focus and country-event subtype references resolve;
        # only the intentionally missing focus should fail typed ID validation.
        self.assertEqual(len([r for r in missing if 'type focus' in r['message']]),1)
        self.assertFalse(any('event.country_event' in row['message'] for row in missing))
        self.assertTrue(any(row['code']=='CW242' for row in rows),messages)

    def test_engine_localisation_diagnostics_and_source_positions(self):
        rows=self.result['diagnostics']
        self.assertTrue(any(row['category']=='localisation' for row in rows))
        self.assertTrue(all(row['line']>=1 and row['column']>=0 for row in rows))
        self.assertTrue(all(not row['file'].startswith('C:/__') for row in rows))

    def test_parse_errors_are_exposed_without_claiming_full_coverage(self):
        file=Path(self.temp.name)/'broken.txt';file.write_text('focus_tree={',encoding='utf-8')
        result=analyze({'common/national_focus/broken.txt':str(file)})
        self.assertTrue(result['parser_errors'])
        self.assertEqual(result['parser_errors'][0]['file'],'common/national_focus/broken.txt')
        self.assertEqual(result['parser_errors'][0]['classification'],'rejected_by_both')
        self.assertTrue(result['parser_errors'][0]['jomini_error'])

    def test_jomini_lexical_compatibility_preserves_tags_comments_unicode_and_locations(self):
        file=Path(self.temp.name)/'compatibility.txt'
        source='''country_event={id=test.2 is_triggered_only=yes
            trigger={original_tag = RGB resistance > 1}
            option={name=OPTION
                set_variable={var = variable value = \u00b445}
                if = # comment after operator
                {limit={always=perhaps} add_stability=0.1}
                custom_effect_tooltip = RGB_democratic_RU_Gen
            }
        }'''
        file.write_text(source,encoding='utf-8')
        result=analyze({'events/compatibility.txt':str(file)})
        self.assertEqual(result['parser_errors'],[])
        self.assertEqual(file.read_text(encoding='utf-8'),source)
        self.assertEqual(result['parser_compatibility'],1)
        self.assertEqual(result['input_adjustments'][0]['file'],'events/compatibility.txt')
        row=next(row for row in result['diagnostics'] if row['line']==6 and 'perhaps' in row['message'])
        self.assertEqual(row['line'],6)
        self.assertEqual(row['column'],source.splitlines()[5].index('always'))
        scope=next(r for r in result['diagnostics'] if r['code']=='CW247')
        self.assertEqual(scope['column'],source.splitlines()[1].index('resistance'))
        file.write_text('EXAMPLE={name="Example" unique={Bare tail" } #comment"\n}',encoding='utf8')
        result=analyze({'common/units/names_divisions/example.txt':str(file)})
        self.assertEqual(result['parser_errors'],[])

    def test_non_script_documents_are_classified_and_missing_values_still_fail(self):
        file=Path(self.temp.name)/'readme.txt';file.write_text('Copyright { invalid as a script',encoding='utf-8')
        result=analyze({'gfx/fonts/korean/OFL_example.txt':str(file),'interface/credits.txt':str(file)})
        self.assertEqual(result['parser_errors'],[])
        self.assertEqual(len(result['non_script_files']),2)
        result=analyze({'gfx/particles/fixture.txt':str(file)})
        self.assertTrue(result['parser_errors'])
        file.write_text('particle={force=}',encoding='utf-8')
        self.assertTrue(analyze({'gfx/particles/fixture.txt':str(file)})['parser_errors'])
