"""Geographic resolution uses installed data, never a city-to-state guess list."""
import copy
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from hoi4companion.operation_import import operation_catalog, bind_operations
from hoi4companion.catalog_export import for_ai
from hoi4companion.contracts import normalise_state
from hoi4companion.guide_importer import Importer
from hoi4companion.guide_runtime import evaluate


def catalog():
    return [
        {'kind':'building','id':'arms_factory','name':'Military Factory','location_scope':'state'},
        {'kind':'building','id':'bunker','name':'Land Fort','location_scope':'province'},
        {'kind':'state','id':'64','name':'Brandenburg','aliases':['Brandeburgo'],
         'geography':{'province_ids':[6521,3499], 'cities':[
             {'province_id':6521,'name':'Berlin','localized_name':'Berlino','aliases':['Berlino']}]}},
        {'kind':'state','id':'77','name':'Other State',
         'geography':{'province_ids':[123], 'cities':[]}}]


def bind(params, entries=None):
    guide={'tracks':{'operations':[{'key':'build','action':{'kind':'manual',
        'params':{'category':'construction','quantity':2,'quantity_mode':'additional',**params}}}]}}
    reports=bind_operations(guide, entries if entries is not None else catalog())
    return guide['tracks']['operations'][0],reports[0]


class GeographyTests(unittest.TestCase):
    def test_authoring_and_v1_revalidation_share_the_resolver_and_runtime(self):
        importer=Importer(catalog)
        text='Build a land fort in Berlin.'
        loaded=importer.document({'format':'hoi4-guide-authoring','authoring_schema_version':1,
            'sources':[{'id':'source','type':'manual','text':text,'segments':[]}],
            'guide':{'id':'geographic','title':'Any country','steps':[{
                'key':'fort','type':'build','building':'Land Fort','state':'Berlin',
                'quantity':{'mode':'add','value':1},
                'evidence':[{'source_id':'source','basis':'explicit','quote':text}]}]}})
        report=importer.preview(loaded['guide'],loaded['sources'])
        self.assertTrue(report['valid'],report['errors'])
        guide=report['guide']
        self.assertEqual(importer.preview(guide,loaded['sources'])['guide'],guide)
        state={'schema_version':1,'usable':True,'facts':{'construction.queued':{
            'status':'calculated','source':'fixture','value':['state:64:bunker','province:3499:bunker']}}}
        progress={'schema_version':1,'guide_id':guide['id'],'run_id':'r','checks':{}}
        self.assertNotEqual(evaluate(guide,state,progress)['steps'][0]['status'],'active')
        state['facts']['construction.queued']['value'].append('province:6521:bunker')
        row=evaluate(guide,state,progress)['steps'][0]
        self.assertEqual(row['status'],'active')
        self.assertNotEqual(row['status'],'completed')

    def test_city_maps_to_state_for_factories_and_retains_source_name(self):
        for name in ('Berlin','Berlino',' BERLIN '):
            row,report=bind({'building_name':'Military Factory','state_name':name})
            params=row['action']['params']
            self.assertEqual(params['state_id'],64)
            self.assertEqual(params['state_name'],name)
            self.assertNotIn('province_id',params)
            self.assertEqual(row['started_when']['value'],'state:64:arms_factory')
            self.assertEqual(report['location']['via'],'city')
            self.assertEqual(report['location']['province_id'],6521)
            self.assertEqual(report['monitoring'],'automatic')

    def test_provincial_orders_use_the_city_province_not_state_wide_membership(self):
        row,report=bind({'building_name':'Land Fort','state_name':'Berlin'})
        self.assertEqual(row['action']['params']['province_id'],6521)
        self.assertEqual(row['started_when']['value'],'province:6521:bunker')
        self.assertEqual(report['monitoring'],'start_only')
        self.assertNotIn('completion',row)  # No province level exposed by acquisition.
        row,report=bind({'building_name':'Land Fort','state_name':'Brandenburg'})
        self.assertNotIn('started_when',row)
        self.assertEqual(report['reason'],'province_required')

    def test_province_id_resolves_its_state_without_needing_a_city(self):
        row,_=bind({'building_id':'bunker','province_id':3499})
        self.assertEqual(row['action']['params']['state_id'],64)
        self.assertEqual(row['started_when']['value'],'province:3499:bunker')

    def test_conflicting_or_unknown_ids_never_fall_back_to_the_name(self):
        for params in ({'state_id':77,'state_name':'Berlin'},
                       {'state_id':999,'state_name':'Berlin'},
                       {'state_name':'Berlin','province_id':123},
                       {'state_id':64,'province_id':999},
                       {'province_id':True}):
            row,report=bind({'building_id':'arms_factory',**params})
            self.assertNotIn('started_when',row,params)
            self.assertEqual(report['monitoring'],'manual')

    def test_ambiguous_cities_can_be_disambiguated_by_state_but_not_first_match(self):
        entries=catalog()
        entries[-1]['geography']['cities']=[{'province_id':123,'name':'Berlin','aliases':[]}]
        row,report=bind({'building_id':'arms_factory','state_name':'Berlin'},entries)
        self.assertNotIn('started_when',row)
        self.assertEqual(report['reason'],'location_ambiguous')
        row,_=bind({'building_id':'bunker','state_name':'Berlin','state_id':64},entries)
        self.assertEqual(row['started_when']['value'],'province:6521:bunker')
        # A state-wide building can accept several cities within the same state.
        entries=catalog();entries[2]['geography']['cities'].append({'province_id':3499,'name':'Berlin'})
        row,_=bind({'building_id':'arms_factory','state_name':'Berlin'},entries)
        self.assertEqual(row['action']['params']['state_id'],64)
        row,report=bind({'building_id':'bunker','state_name':'Berlin'},entries)
        self.assertNotIn('started_when',row)
        self.assertEqual(report['reason'],'location_ambiguous')

    def test_shared_province_membership_is_not_treated_as_known(self):
        entries=catalog();entries[-1]['geography']['province_ids'].append(6521)
        for params in ({'building_id':'bunker','province_id':6521},
                       {'building_id':'arms_factory','state_name':'Berlin'}):
            row,report=bind(params,entries)
            self.assertNotIn('started_when',row)
            self.assertEqual(report['reason'],'location_ambiguous')

    def test_mod_precedence_city_localization_and_building_scope_are_read_from_files(self):
        with TemporaryDirectory() as directory:
            game=Path(directory)/'game';mod=Path(directory)/'mod'
            for root in (game,mod):
                (root/'history/states').mkdir(parents=True)
            (game/'common/buildings').mkdir(parents=True)
            (game/'common/buildings/types.txt').write_text('buildings={arms_factory={level_cap={state_max=20}} bunker={level_cap={province_max=10}}}')
            (game/'history/states/64-test.txt').write_text('state={id=64 provinces={6521} history={victory_points={6521 50}}}')
            (mod/'history/states/64-test.txt').write_text('state={id=64 provinces={123} history={victory_points={123 5}}}')
            entries,warnings=operation_catalog(game,[mod],{'VICTORY_POINTS_123':'Città mod'},
                {'STATE_64':'Mod State','VICTORY_POINTS_123':'Mod City','VICTORY_POINTS_6521':'Berlin'})
            self.assertEqual(warnings,[])
            region=next(e for e in entries if e['kind']=='state')
            self.assertEqual(region['geography']['province_ids'],[123])
            self.assertEqual(region['geography']['cities'][0]['localized_name'],'Città mod')
            self.assertEqual({e['id']:e['location_scope'] for e in entries if e['kind']=='building'},
                             {'arms_factory':'state','bunker':'province'})
            row,_=bind({'building_id':'arms_factory','state_name':'Città mod'},entries)
            self.assertEqual(row['action']['params']['state_id'],64)
            row,_=bind({'building_id':'arms_factory','state_name':'Berlin'},entries)
            self.assertNotIn('started_when',row)

    def test_geography_stays_local_and_catalog_is_not_mutated(self):
        entries=catalog();before=copy.deepcopy(entries)
        bind({'building_id':'arms_factory','state_name':'Berlin'},entries)
        exported=for_ai(entries)
        self.assertNotIn('geography',exported[2])
        self.assertEqual(entries,before)

    def test_queue_adapter_exposes_province_keys_only_for_observed_locations(self):
        line={'id':'x','building_id':'bunker','location_status':'observed','state_id':64,
              'province_id':6521,'amount':1,'active_factories':0,'built_level':0,'repair_levels':0,'conversion':False}
        value={'usable':True,'sample':{'country':'GER','country_overview':{
            'country':'GER','construction':{'status':'observed','lines':[line]}}}}
        fact=normalise_state(value)['facts']['construction.queued']
        self.assertEqual(fact['value'],['building:bunker','province:6521:bunker','state:64:bunker'])
        line['province_id']=None
        self.assertEqual(normalise_state(value)['facts']['construction.queued']['value'],['building:bunker','state:64:bunker'])
        line['location_status']='unknown'
        self.assertEqual(normalise_state(value)['facts']['construction.queued']['status'],'unknown')


if __name__=='__main__':unittest.main()
