from unittest import TestCase
from copy import deepcopy
from hoi4companion.backend_core import _request
from hoi4companion.operation_import import bind_operations
from test_geography import catalog

class NativeConstructionBindingTests(TestCase):
    def test_projects_agency_state_and_province_goals_match_existing_bindings(self):
        entries=catalog()+[{'kind':'project','id':'project','name':'Engineering Vehicles'}]
        cases=[{'operation_id':'create_intelligence_agency'},
               {'category':'projects','project_name':'Engineering Vehicles'},
               {'category':'construction','building_name':'Military Factory','state_name':'Berlin','quantity':2,'quantity_mode':'additional'},
               {'category':'construction','building_id':'bunker','state_name':'Berlin','quantity':2,'quantity_mode':'additional'},
               {'category':'construction','building_id':'bunker','province_selector':{'kind':'border','country':'TST'},'quantity':3,'quantity_mode':'target_total'},
               {'category':'construction','building_id':'arms_factory','state_id':999,'quantity':2,'quantity_mode':'target_total'}]
        for params in cases:
            step={'key':'build','action':{'kind':'manual','params':params}};guide={'tracks':{'operations':[deepcopy(step)]}}
            report=bind_operations(guide,entries)[0]
            result=_request(0,'import_bind_foundation',step=step,entries=entries)
            with self.subTest(params=params):
                self.assertTrue(result['handled'])
                self.assertEqual(result['step'],guide['tracks']['operations'][0])
                self.assertEqual(result['reason'],report['reason'])
                self.assertEqual(result['location'] if result['location'] and 'state_id' in result['location'] else None,report.get('location'))
