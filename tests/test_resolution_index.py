import copy
import unittest
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
        sizes=[len(index._ids),len(index._names),len(index._aliases),len(index._prefixes)]
        def lookup(i):
            context=ResolutionContext(country=str(i))
            self.assertEqual(index.matches('missing-'+str(i),('state',),context),[])
            self.assertEqual(index.matches('Brandenburg',('state',),context)[0]['id'],'64')
        with ThreadPoolExecutor(max_workers=8) as pool:list(pool.map(lookup,range(150)))
        self.assertLessEqual(len(index._contexts),64)
        self.assertEqual([len(index._ids),len(index._names),len(index._aliases),len(index._prefixes)],sizes)

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
