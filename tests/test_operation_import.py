import copy
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from hoi4companion.guide_importer import preview
from hoi4companion.guide_runtime import evaluate
from hoi4companion.import_sources import acquire_source
from hoi4companion.operation_import import operation_catalog


CATALOG=[{'kind':'state','id':'77','name':'Example State','localized_name':'Regione Esempio','aliases':[]},
         {'kind':'building','id':'infrastructure','name':'Infrastructure','aliases':[]},
         {'kind':'building','id':'arms_factory','name':'Military Factory','aliases':[]},
         {'kind':'building','id':'land_facility','name':'Land Warfare Facility','aliases':[]}]


def document(params):
    src=acquire_source(text='Build the buildings and create an agency.')
    guide={'schema_version':1,'id':'operations-import','title':'Operations','applies_to':{'country':'TST'},
           'sources':[{'id':src['id'],'type':'manual'}], 'tracks':{'operations':[
               {'key':'order','action':{'kind':'manual','params':params},
                'evidence':[{'source_id':src['id'],'basis':'explicit','quote':src['text']}]}]}}
    return guide,[src]


def state(level=3,queued=(),creating=False,created=False,usable=True,country='TST'):
    return {'schema_version':1,'usable':usable,'session_id':'s','facts':{
        key:{'status':'observed','value':value,'source':'fixture'} for key,value in {
            'country':country,'construction.queued':list(queued),
            'country.states.items.77.infrastructure':level,
            'country.states.items.77.military_factories':level,
            'country.intelligence.in_creation':creating,'country.intelligence.is_created':created}.items()}}


class OperationImportTests(unittest.TestCase):
    def test_state_names_include_native_country_names_without_hardcoded_regions(self):
        with TemporaryDirectory() as directory:
            game=Path(directory); (game/'history'/'states').mkdir(parents=True)
            (game/'history'/'states'/'77-example.txt').write_text('state={id=77 name="STATE_77"}')
            entries,warnings=operation_catalog(game,[],{'STATE_77':'Regione Esempio'},
                {'STATE_77':'Example State','TST_STATE_77':'National Example'})
            self.assertEqual(warnings,[])
            self.assertIn('National Example',entries[0]['aliases'])
            self.assertEqual(entries[0]['localized_name'],'Regione Esempio')

    def test_binds_target_level_to_existing_live_facts_without_changing_original(self):
        guide,sources=document({'category':'construction','building_name':'Infrastructure',
                                'state_name':'Example State','target_level':5})
        before=copy.deepcopy(guide); report=preview(guide,sources,CATALOG)
        self.assertTrue(report['valid'],report['errors']); self.assertEqual(guide,before)
        step=report['guide']['tracks']['operations'][0]
        self.assertIn('started_when',step); self.assertIn('completion',step)
        self.assertEqual(step['action']['params']['state_id'],77)
        self.assertEqual(report['operations'][0]['monitoring'],'automatic')
        progress={'schema_version':1,'guide_id':guide['id'],'run_id':'r','checks':{}}
        for live,expected in [(state(),'ready'),(state(queued=['state:77:infrastructure']),'active'),
                              (state(level=5),'completed'),(state(queued=[]),'ready')]:
            self.assertEqual(evaluate(report['guide'],live,progress)['steps'][0]['status'],expected)
        self.assertNotEqual(evaluate(report['guide'],state(usable=False),progress)['steps'][0]['status'],'completed')
        self.assertNotEqual(evaluate(report['guide'],state(level=5,country='OTH'),progress)['steps'][0]['status'],'completed')
        self.assertEqual(preview(report['guide'],sources,CATALOG)['guide'],report['guide'])

    def test_additional_factories_need_a_baseline_and_queue_disappearance_is_not_completion(self):
        guide,sources=document({'category':'construction','building_id':'arms_factory','state_id':77,
                                'quantity':2,'quantity_mode':'additional'})
        report=preview(guide,sources,CATALOG); step=report['guide']['tracks']['operations'][0]
        self.assertIn('started_when',step); self.assertIn('completion',step)
        self.assertEqual(report['operations'][0]['monitoring'],'automatic')
        self.assertEqual(report['operations'][0]['reason'],'baseline_from_live')
        progress={'schema_version':1,'guide_id':guide['id'],'run_id':'r','checks':{}}
        self.assertEqual(evaluate(report['guide'],state(queued=['state:77:arms_factory']),progress)['steps'][0]['status'],'active')
        self.assertNotEqual(evaluate(report['guide'],state(level=10),progress)['steps'][0]['status'],'completed')
        guide['tracks']['operations'][0]['action']['params']['baseline_level']=3
        report=preview(guide,sources,CATALOG)
        self.assertEqual(evaluate(report['guide'],state(level=5),progress)['steps'][0]['status'],'completed')

    def test_agency_creation_is_automatic_even_with_manual_action_kind(self):
        guide,sources=document({'category':'projects','project_name':'Intelligence Agency'})
        report=preview(guide,sources,CATALOG)
        self.assertEqual(report['operations'][0]['monitoring'],'automatic')
        progress={'schema_version':1,'guide_id':guide['id'],'run_id':'r','checks':{'order':True}}
        for creating,created,expected in [(False,False,'ready'),(True,False,'active'),(False,True,'completed')]:
            self.assertEqual(evaluate(report['guide'],state(creating=creating,created=created),progress)['steps'][0]['status'],expected)

    def test_unknown_or_ambiguous_locations_never_bind_to_a_different_state(self):
        guide,sources=document({'category':'construction','building_name':'Infrastructure',
                                'state_name':'Unknown','target_level':5})
        for entities in [CATALOG,CATALOG+[{'kind':'state','id':'88','name':'Example State','aliases':[]}]]:
            if len(entities)>len(CATALOG):guide['tracks']['operations'][0]['action']['params']['state_name']='Example State'
            report=preview(guide,sources,entities)
            self.assertTrue(report['valid'],report['errors'])
            self.assertNotIn('started_when',report['guide']['tracks']['operations'][0])
            self.assertEqual(report['operations'][0]['monitoring'],'manual')

    def test_preserves_explicit_conditions_and_does_not_parse_free_prose(self):
        guide,sources=document({'category':'construction','instructions':['Build five military factories in Example State']})
        report=preview(guide,sources,CATALOG)
        self.assertNotIn('completion',report['guide']['tracks']['operations'][0])
        guide['tracks']['operations'][0]['completion']={'op':'check','key':'author-check'}
        report=preview(guide,sources,CATALOG)
        self.assertEqual(report['guide']['tracks']['operations'][0]['completion'],{'op':'check','key':'author-check'})
        self.assertFalse(any('istruzione manuale;' in w for w in report['warnings']))
