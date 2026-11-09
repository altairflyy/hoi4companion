"""Native catalog parity; no game launch, state reading or guide installation."""
import copy
import unittest

from hoi4companion.catalog_core import NativeCatalog,engine_info
from hoi4companion._resolution_reference import CatalogIndex as Reference,ResolutionContext


ENTRIES=[
 {'kind':'focus','id':'one','name':'Reinstate the Herrenhaus','scope':'a','countries':['AUS'],'aliases':['The Council']},
 {'kind':'focus','id':'two','name':'Reinstate the House','scope':'b','countries':['GER']},
 {'kind':'focus','id':'three','name':'Build the Rur Dam','scope':'a','countries':['AUS']},
 {'kind':'focus','id':'future','name':'An [AUS.GetAdjective] Future','scope':'a','countries':['AUS'],'aliases':['An Austrian Future']},
 {'kind':'technology','id':'c1','name':'Concentrated Industry I'},
 {'kind':'technology','id':'c2','name':'Concentrated Industry II'},
 {'kind':'technology','id':'at','name':'Infantry Anti-Tank I','aliases':['Panzerfaust 30']},
 {'kind':'law','id':'l1','name':'Mobilization','group':'economy','country_condition':{'op':'tag','value':'GER'}},
 {'kind':'law','id':'l2','name':'Mobilization','group':'conscription','country_condition':{'op':'unknown'}},
 {'kind':'event','id':'e','name':'Ultimatum','country_condition':{'op':'tag','value':'GER'}},
 {'kind':'event_option','id':'e.a','name':'Accept','event_id':'e'},
 {'kind':'event','id':'other','name':'Ultimatum','country_condition':{'op':'tag','value':'POL'}},
 {'kind':'event_option','id':'other.a','name':'Accept','event_id':'other'},
 {'kind':'focus','id':'suppress','name':'Suppress Subjects','continuous':True,'scope_independent':True},
 {'kind':'technology','id':'utf','name':'The Kırıkkale Tank','aliases':['Mitragliatrici pesanti','Łódź']},
]


class NativeCatalogTests(unittest.TestCase):
    def test_unicode_normalization_and_typographic_variants_preserve_reference_contract(self):
        from hoi4companion.research_upgrades import normal_name
        import unicodedata
        samples=['','I',' I','I II','Heavy machineguns II','Anti--aircraft I','Machinegunship',
                 'IMPROVED SMALL AIRFRAME IV','Tools\u001fII','Ｍａｃｈｉｎｅ ｇｕｎｓ','İ ı ß æ œ ø ł',
                 'Machİne Guns II','Infantry Equipment Ⅱ','To  War Plan Orange','ᾲ','Straße']
        samples.extend(chr(n) for n in range(0x110000) if not 0xD800<=n<=0xDFFF and (
            unicodedata.normalize('NFKD',chr(n))!=chr(n) or chr(n).casefold()!=chr(n)))
        with NativeCatalog([]) as native:
            for start in range(0,len(samples),500):
                batch=samples[start:start+500]
                result=native.query('normalize',texts=batch)
                for raw,actual in zip(batch,result):self.assertEqual(actual,normal_name(raw),repr(raw))

    def test_native_identity_precedence_context_typos_and_parent_filters_match_reference(self):
        self.assertEqual(engine_info()['engine'],'rust-catalog-core')
        reference=Reference(ENTRIES,adjectives=['Austrian','Austro-Hungarian'])
        with NativeCatalog(ENTRIES,adjectives=['Austrian','Austro-Hungarian']) as native:
            cases=[('Reinstate the Herrenhaus',['focus']),('The Council',['focus']),
                ('Reinstate The Herrenbuilding',['focus']),('The Rur Dam',['focus']),
                ('Concentrated 1',['technology']),('Panzerfaust',['technology']),
                ('Mobilization',['law']),('Accept',['event_option']),('missing',['focus']),
                ('one',['focus']),('An Austrian/Austro-Hungarian Future',['focus']),
                ('Suppress Subjects/Nothing',['focus']),('The Kirikkale Tank',['technology']),('Łódź',['technology'])]
            for country in (None,'AUS','GER','POL'):
                for scope in (None,'a','b'):
                    context=ResolutionContext(country,scope)
                    for query,kinds in cases:
                        for explicit in (False,True):
                            with self.subTest(query=query,country=country,scope=scope,explicit=explicit):
                                expected=reference.matches(query,kinds,context,explicit=explicit,typos=True)
                                self.assertEqual(native.query('matches',query=query,kinds=kinds,country=country,scope=scope,explicit=explicit,typos=True),
                                    [i for i,row in enumerate(ENTRIES) if row in expected])
            expected=reference.matches('Accept',('event_option',),ResolutionContext(country='GER'),event_id='e')
            self.assertEqual(native.query('matches',query='Accept',kinds=['event_option'],country='GER',filters={'event_id':'e'}),[10])
            self.assertEqual(len(expected),1)

    def test_review_candidates_and_word_variants_never_become_bindings(self):
        reference=Reference(ENTRIES)
        with NativeCatalog(ENTRIES) as native:
            for query in ('Reinstate The Herrenbuilding','The Rur Dam','Reinstat the Herrenhaus','zzzz zzzz','Panzerfaust'):
                context=ResolutionContext(country='AUS',scope='a')
                self.assertEqual(native.query('focus_words',query=query,kinds=['focus'],country='AUS',scope='a'),
                    [[ENTRIES.index(row),label] for row,label in reference.focus_word_candidates(query,context)])
                result=native.query('suggest',query=query,kinds=['focus'],country='AUS',scope='a',limit=6)
                expected=reference.suggest(query,('focus',),context)
                self.assertEqual([{**copy.deepcopy(ENTRIES[row['index']]),'suggestion':row['suggestion']} for row in result],expected)

    def test_close_rejects_queries_and_catalog_never_uses_observed_progress(self):
        native=NativeCatalog(ENTRIES)
        before=native.query('stats');native.query('matches',query='unknown',kinds=['focus'])
        after=native.query('stats')
        for key in ('rows','ids','names','aliases','prefixes'):self.assertEqual(before[key],after[key])
        native.close();native.close()
        with self.assertRaisesRegex(ValueError,'closed'):native.query('rows',kinds=['focus'])
