from unittest import TestCase
from copy import deepcopy
from hoi4companion.backend_core import _request
from hoi4companion._operation_import_reference import _bind_production
from hoi4companion.resolution import CatalogIndex

class NativeProductionBindingTests(TestCase):
    def test_declared_counts_remaining_priority_and_layout_match_existing_compiler(self):
        entries=[{'kind':'equipment','id':'inf','name':'Infantry Equipment','equipment_ids':['rifle1','rifle2'],'family_id':'inf','is_archetype':True},
                 {'kind':'equipment','id':'train','name':'Train','equipment_ids':['train']},
                 {'kind':'equipment','id':'tank','name':'Tank','equipment_ids':['tank']}]
        library={'designs':[{'id':'recipe','name':'T-I1','equipment_id':'tank','category':'land','modules':{'gun':'cannon'},'upgrades':{}}]}
        index=CatalogIndex(entries,reference_designs=library)
        cases=[{'equipment_name':'Infantry Equipment','requested_factories':10},
               {'equipment_name':'Train','assigned_factories':1},
               {'equipment_name':'Infantry Equipment','quantity_mode':'remaining','production_category':'military','production_priority':'last'},
               {'equipment_name':'T-I1','requested_factories':3},
               {'equipment_name':'Train','requested_factories':True},
               {'production_targets':[{'equipment_name':'Train','requested_factories':1},{'equipment_name':'Infantry Equipment','quantity_mode':'remaining','production_category':'military'}]}]
        for params in cases:
            step={'key':'production','action':{'kind':'manual','params':{'category':'production',**params}}};expected=deepcopy(step)
            reason=_bind_production(expected,index,'TST')
            actual=_request(0,'import_bind_production',step=step,country='TST',library=library,**index.native_import_args())
            with self.subTest(params=params):self.assertEqual(actual,{'step':expected,'reason':reason})
