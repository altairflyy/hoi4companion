import copy
import unittest


class NumberedAliasTests(unittest.TestCase):
    def test_slash_adjectives_in_a_scripted_name_are_one_native_focus_not_two_branches(self):
        from hoi4companion.resolution import CatalogIndex,ResolutionContext
        entries=[{'kind':'focus','id':'future','name':'An [AUS.GetAdjective] Future','countries':['AUS'],
                  'aliases':['An Austrian Future'],'scope':'austria'}]
        index=CatalogIndex(entries,adjectives=['Austrian','Austro-Hungarian'])
        query='An Austrian/Austro-Hungarian Future'
        self.assertEqual([r['id'] for r in index.matches(query,('focus',),ResolutionContext(country='AUS'))],['future'])
        self.assertEqual(index.matches(query,('focus',),ResolutionContext(country='GER')),[])
        self.assertEqual(index.matches('An Austrian/Unknown Future',('focus',)),[])
    def test_focus_route_prefix_and_turkish_transliteration_preserve_native_identity(self):
        from hoi4companion.resolution import CatalogIndex
        index=CatalogIndex([{'kind':'focus','id':'real_to','name':'To Steal an Army'},
                            {'kind':'focus','id':'endpoint','name':'War Plan Orange'},
                            {'kind':'focus','id':'turkish','name':'The Kırıkkale Tank'}])
        self.assertEqual(index.matches('To Steal an Army',('focus',))[0]['id'],'real_to')
        self.assertEqual(index.matches('Steal an Army',('focus',))[0]['id'],'real_to')
        self.assertEqual(index.matches('To War Plan Orange',('focus',))[0]['id'],'endpoint')
        self.assertEqual(index.matches('The Kirikkale Tank',('focus',))[0]['id'],'turkish')
        self.assertEqual(index.matches('To endpoint',('focus',),explicit=True),[])
    def test_country_title_requires_distinct_focus_support_and_rejects_warlord_mismatch(self):
        from hoi4companion.resolution import CatalogIndex
        from hoi4companion.guide_importer import preview
        from hoi4companion.import_sources import acquire_source
        rows=[{'kind':'focus','id':f'{tag}{n}','scope':tag,'countries':[tag],
               'name':name,'prerequisites':[],'mutually_exclusive':[]} for tag,n,name in
              [('ITA',1,'Unique Italian One'),('ITA',2,'Unique Italian Two'),
               ('GER',1,'Unique German One'),('GER',2,'Unique German Two')]]
        index=CatalogIndex(rows,country_names={'ITA':{'Italy'},'GER':{'Germany'},'CHI':{'China'}})
        src=acquire_source(text='Unique Italian One\nUnique Italian Two')
        guide={'id':'country-title','title':'ITALY (Fascist) Guide','sources':[{'id':src['id'],'type':'manual'}],
               'tracks':{'focus':[{'key':str(n),'action':{'kind':'select_focus','entity':{'kind':'focus','id':name}},
                   'evidence':[{'source_id':src['id'],'basis':'explicit','quote':name}]} for n,name in enumerate(src['text'].splitlines())]}}
        result=preview(guide,[src],index)
        self.assertEqual(result['guide']['applies_to']['country'],'ITA')
        guide['title']='CHINA Sichuan guide'
        self.assertNotIn('country',preview(guide,[src],index)['guide']['applies_to'])
        guide['title']='GERMANY Guide'
        self.assertNotIn('country',preview(guide,[src],index)['guide']['applies_to'])
    def test_similarity_candidates_are_review_only_and_do_not_bind(self):
        from hoi4companion.resolution import CatalogIndex,ResolutionContext
        index=CatalogIndex([{'kind':'focus','id':'council','name':'Reinstate the Herrenhaus','countries':['AUS']},
                            {'kind':'focus','id':'other','name':'Reinstate the House','countries':['GER']}])
        context=ResolutionContext(country='AUS')
        self.assertEqual(index.matches('Reinstate The Herrenbuilding',('focus',),context,typos=True),[])
        result=index.suggest('Reinstate The Herrenbuilding',('focus',),context)
        self.assertEqual([r['id'] for r in result],['council'])
        self.assertTrue(result[0]['suggestion']['requires_review'])
        result[0]['id']='corrupted'
        self.assertEqual(index.suggest('Reinstate The Herrenbuilding',('focus',),context)[0]['id'],'council')
        self.assertEqual(index.suggest('zzzz zzzz',('focus',)),[])
        empty=CatalogIndex([{'kind':'focus','id':'blank_title','name':'','aliases':[]}])
        self.assertEqual(empty.suggest('blank title',('focus',)),[])
    def test_family_abbreviations_preserve_explicit_level_and_native_ids(self):
        from hoi4companion.resolution import CatalogIndex
        index=CatalogIndex([{'kind':'technology','id':'c1','name':'Concentrated Industry I'},
                            {'kind':'technology','id':'c2','name':'Concentrated Industry II'},
                            {'kind':'technology','id':'e1','name':'Engineer Company I'}])
        self.assertEqual([r['id'] for r in index.matches('Concentrated 1',('technology',))],['c1'])
        self.assertEqual([r['id'] for r in index.matches('Eng Company II',('technology',))],[])
        self.assertEqual([r['id'] for r in index.matches('Eng Company 1',('technology',))],['e1'])
        self.assertEqual(index.matches('Concentrated 1',('technology',),explicit=True),[])
        self.assertEqual(index.matches('c1',('technology',),explicit=True)[0]['id'],'c1')
    def test_long_model_name_can_omit_only_numeric_suffix_without_choosing_among_versions(self):
        from hoi4companion.resolution import CatalogIndex
        rows=[{'kind':'technology','id':'at','name':'Infantry Anti-Tank I','aliases':['Panzerfaust 30']},
              {'kind':'technology','id':'rocket','name':'Other','aliases':['Fliegende Panzerfaust']}]
        index=CatalogIndex(rows)
        self.assertEqual([r['id'] for r in index.matches('Panzerfaust',('technology',))],['at'])
        self.assertEqual(index.matches('Panzer',('technology',)),[])
        self.assertEqual(index.matches('Panzerfaust',('technology',),explicit=True),[])
        rows.append({'kind':'technology','id':'at2','name':'Infantry Anti-Tank II','aliases':['Panzerfaust 60']})
        self.assertEqual(len(CatalogIndex(rows).matches('Panzerfaust',('technology',))),2)
from concurrent.futures import ThreadPoolExecutor
from threading import RLock
from unittest.mock import patch

from hoi4companion.import_sources import acquire_source
from hoi4companion.guide_importer import Importer, preview
from hoi4companion.operation_import import bind_operations
from hoi4companion.api import Store


def document(kind,query,country='GER'):
    src=acquire_source(text=query)
    return {'schema_version':1,'id':'context','title':'Context','applies_to':{'country':country},
        'sources':[{'id':src['id'],'type':'manual'}],'tracks':{'main':[{'key':'a',
        'action':{'kind':kind,'entity':{'kind':'focus' if kind=='select_focus' else 'technology','id':query}},
        'evidence':[{'source_id':src['id'],'basis':'explicit','quote':query}]}]}},[src]


class ResolutionContextTests(unittest.TestCase):
    def test_national_conditions_filter_political_names_without_id_prefix_guesses(self):
        catalog=[{'kind':'law','id':'generic','name':'Mobilization','group':'conscription',
                  'country_condition':{'op':'tag','value':'GER'}},
                 {'kind':'law','id':'GER_misleading','name':'Mobilization','group':'conscription',
                  'country_condition':{'op':'tag','value':'POL'}}]
        guide={'applies_to':{'country':'GER'},'tracks':{'operations':[{'key':'law',
            'action':{'kind':'manual','params':{'category':'politics','law_name':'Mobilization'}}}]}}
        before=copy.deepcopy(catalog)
        report=bind_operations(guide,catalog)
        row=guide['tracks']['operations'][0]
        self.assertEqual(row['action']['params']['law_id'],'generic')
        self.assertEqual(report[0]['monitoring'],'automatic')
        excluded=report[0]['bindings'][0]['excluded_candidates']
        self.assertEqual(excluded[0]['entity']['id'],'GER_misleading')
        self.assertEqual(catalog,before)

    def test_events_and_options_use_parent_and_country_as_a_single_context(self):
        catalog=[{'kind':'event','id':'g','name':'Ultimatum','country_condition':{'op':'tag','value':'GER'}},
                 {'kind':'event','id':'p','name':'Ultimatum','country_condition':{'op':'tag','value':'POL'}},
                 {'kind':'event_option','id':'g.a','event_id':'g','name':'Accept'},
                 {'kind':'event_option','id':'p.a','event_id':'p','name':'Accept'}]
        guide={'applies_to':{'country':'GER'},'tracks':{'operations':[{'key':'event',
            'action':{'kind':'manual','params':{'category':'politics','event_name':'Ultimatum','event_option':'Accept'}}}]}}
        report=bind_operations(guide,catalog)
        params=guide['tracks']['operations'][0]['action']['params']
        self.assertEqual((params['event_id'],params['event_option_id']),('g','g.a'))
        self.assertEqual(report[0]['monitoring'],'start_only')
        # An explicitly contradictory ID stays unresolved, even if the name fits.
        params.update(event_id='p');report=bind_operations(guide,catalog)
        self.assertEqual(report[0]['monitoring'],'manual')

    def test_unknown_country_conditions_and_original_tags_cannot_disambiguate(self):
        for condition in ({'op':'unknown'},{'op':'original_tag','value':'POL'}):
            catalog=[{'kind':'law','id':'a','name':'Law','group':'economy','country_condition':condition},
                     {'kind':'law','id':'b','name':'Law','group':'economy','country_condition':{'op':'tag','value':'GER'}}]
            guide={'applies_to':{'country':'GER'},'tracks':{'operations':[{'key':'law',
                'action':{'kind':'manual','params':{'law_name':'Law'}}}]}}
            self.assertEqual(bind_operations(guide,catalog)[0]['monitoring'],'manual')

    def test_event_parent_context_precedes_name_ranking(self):
        catalog=[{'kind':'event','id':'e','name':'Event'},
                 {'kind':'event_option','id':'wanted','event_id':'e','name':'Agreed','aliases':['Accept']},
                 {'kind':'event_option','id':'other','event_id':'x','name':'Accept'}]
        guide={'tracks':{'operations':[{'key':'event','action':{'kind':'manual','params':{
            'event_id':'e','event_option':'Accept'}}}]}}
        report=bind_operations(guide,catalog)
        self.assertEqual(report[0]['monitoring'],'start_only')
        self.assertEqual(guide['tracks']['operations'][0]['action']['params']['event_option_id'],'wanted')


class ResolutionIndexTests(unittest.TestCase):
    def test_concurrent_queries_are_bounded_and_unknown_names_do_not_grow_name_indexes(self):
        from hoi4companion.resolution import CatalogIndex, ResolutionContext
        index=CatalogIndex([{'kind':'state','id':'64','name':'Brandenburg'}])
        sizes=index.diagnostics()
        def lookup(i):
            context=ResolutionContext(country=str(i))
            self.assertEqual(index.matches('missing-'+str(i),('state',),context),[])
            self.assertEqual(index.matches('Brandenburg',('state',),context)[0]['id'],'64')
        with ThreadPoolExecutor(max_workers=8) as pool:list(pool.map(lookup,range(150)))
        after=index.diagnostics()
        self.assertEqual(after['engine'],'rust-catalog-core')
        self.assertLessEqual(after['contexts'],64)
        for key in ('ids','names','aliases','prefixes'):self.assertEqual(after[key],sizes[key])

    def test_indexes_preserve_duplicates_name_precedence_scope_and_explicit_id(self):
        from hoi4companion.resolution import CatalogIndex, ResolutionContext
        rows=[{'kind':'focus','id':'one','name':'Same','scope':'a','countries':['GER']},
              {'kind':'focus','id':'two','name':'Same','scope':'b','countries':['POL']},
              {'kind':'focus','id':'alias','name':'Other','scope':'a','aliases':['Same']}]
        before=copy.deepcopy(rows);index=CatalogIndex(rows)
        self.assertEqual([r['id'] for r in index.matches('Same',('focus',))],['one','two'])
        context=ResolutionContext(country='GER',scope='a')
        self.assertEqual([r['id'] for r in index.matches('Same',('focus',),context)],['one'])
        self.assertEqual(index.matches('missing',('focus',),context,explicit=True),[])
        self.assertEqual(rows,before)
        rows[0]['name']='Mutated';self.assertEqual(index.matches('Same',('focus',),context)[0]['name'],'Same')
        self.assertEqual(index.excluded('Same',('focus',),context)[0]['reason'],'country_or_scope_conflict')

    def test_shared_index_matches_legacy_rules_and_retains_future_targets(self):
        from hoi4companion.resolution import CatalogIndex
        from hoi4companion.research_upgrades import name_matches
        rows=[{'kind':'technology','id':'a','name':'Fulfil the Research Act','aliases':['Alias']},
              {'kind':'technology','id':'b','name':'Other Research','aliases':['Alias']}]
        index=CatalogIndex(rows)
        for query in ('a','Alias','Fulfill the Research Act','Fulfil the','Unknown','a_'):
            self.assertEqual(index.matches(query,('technology',),typos=True),name_matches(query,rows))
        guide,sources=document('research','a')
        report=preview(guide,sources,index)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual(report['guide']['tracks']['main'][0]['target'],'a')
        report['resolutions'][0]['candidates'][0]['name']='Corrupted'
        self.assertEqual(preview(guide,sources,index)['resolutions'][0]['candidates'][0]['name'],rows[0]['name'])

    def test_store_reuses_indexes_and_invalidates_at_catalog_reload(self):
        store=Store.__new__(Store);store.lock=RLock();store._analysis_generation=1
        store.trees={};store.technologies={'technologies':{}};store.labels={};store.reference_labels={}
        store.operation_entities=[{'kind':'state','id':'64','name':'Brandenburg'}]
        with patch('hoi4companion.api.catalog_entities',return_value=[]) as build:
            first=store.resolution_index();self.assertIs(store.resolution_index(),first)
            self.assertEqual(build.call_count,1)
            exported=store.import_catalog();exported[0]['name']='Corrupted'
            self.assertEqual(store.import_catalog()[0]['name'],'Brandenburg')
            store._analysis_generation+=1
            store.operation_entities=[{'kind':'state','id':'77','name':'New State'}]
            second=store.resolution_index();self.assertIsNot(first,second)
            self.assertEqual(build.call_count,2)
            self.assertEqual(second.matches('New State',('state',))[0]['id'],'77')

    def test_importer_uses_revision_owned_index_but_not_mutable_result_cache(self):
        from hoi4companion.resolution import CatalogIndex
        rows=[{'kind':'technology','id':'a','name':'Research'}]
        index=CatalogIndex(rows);calls=[]
        importer=Importer(lambda:(_ for _ in ()).throw(AssertionError('Uncached catalog called')),
                          resolution_index=lambda:(calls.append(1) or index))
        guide,sources=document('research','Research')
        self.assertTrue(importer.preview(guide,sources)['valid'])
        self.assertTrue(importer.preview(guide,sources)['valid'])
        self.assertEqual(len(calls),2)

    def test_catalog_and_research_are_snapshotted_together_and_evaluated_outside_lock(self):
        from hoi4companion.resolution import CatalogIndex
        locked=[]
        class Lock:
            def __enter__(self):locked.append(True)
            def __exit__(self,*args):locked.pop()
        index=CatalogIndex([{'kind':'technology','id':'a','name':'Research'}])
        def context(guide):
            self.assertTrue(locked);return None
        def snapshot():
            self.assertTrue(locked);return index
        def evaluate(*args,**kwargs):
            self.assertFalse(locked);return {'valid':True}
        importer=Importer(lambda:[],research_context=context,resolution_index=snapshot,catalog_lock=Lock())
        with patch('hoi4companion.guide_importer.preview',side_effect=evaluate):
            self.assertTrue(importer.preview({},[])['valid'])


if __name__=='__main__':unittest.main()
