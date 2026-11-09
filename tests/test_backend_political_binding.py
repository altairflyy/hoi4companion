from copy import deepcopy
from unittest import TestCase
from hoi4companion.backend_core import _request
from hoi4companion._political_import_reference import bind_political

class NativePoliticalBindingTests(TestCase):
    def test_foreign_mission_and_relation_bindings_preserve_qualified_milestones(self):
        from hoi4companion._foreign_interactions_reference import bind
        from hoi4companion.resolution import CatalogIndex
        index=CatalogIndex([],country_names={'TST':['Testland']})
        for params in ({'operation_id':'improve_relations','target_country':'Testland'},
                       {'operation_id':'stop_improving_relations','target_country':'TST'},
                       {'category':'projects','operation_name':'build network','target_country':'TST'},
                       {'operation_id':'build_intel_network','target_country':'TST','network_strength':50}):
            step={'key':'diplomacy','action':{'kind':'manual','params':params}};expected=deepcopy(step)
            bound=bind(expected,index)
            actual=_request(0,'import_bind_foreign',step=step,**index.native_import_args())
            self.assertEqual(actual,{'step':expected,'bound':bound})
    def test_adapter_retains_action_and_parameter_references(self):
        from hoi4companion.political_import import bind_political as bind
        step={'key':'law','action':{'kind':'manual','params':{'law_name':'Civilian Economy'}}}
        action=step['action'];params=action['params']
        bind(step,[{'kind':'law','id':'law','name':'Civilian Economy','group':'economy'}],None)
        self.assertIs(step['action'],action)
        self.assertIs(step['action']['params'],params)
        self.assertEqual(params['law_id'],'law')
    def test_qualified_law_appointment_event_and_unknown_identity_parity(self):
        rows=[{'kind':'law','id':'law','name':'Civilian Economy','group':'economy'},
              {'kind':'advisor','id':'expert','name':'Infantry Expert','slot':'high_command','character_id':'person','countries':['TST']},
              {'kind':'event','id':'event','name':'Event'},
              {'kind':'event_option','id':'option','name':'Accept','event_id':'event'},
              {'kind':'decision','id':'decision','name':'A decision'}]
        for params in ({'law_name':'Civilian Economy'}, {'trait_name':'Infantry Expert','role':'high_command'},
                       {'event_name':'Event','event_option':'Accept'},{'decision_name':'A decision'},
                       {'law_id':'missing','law_name':'Civilian Economy'},{}):
            step={'key':'order','action':{'kind':'manual','params':params}};expected=deepcopy(step);audit=[]
            reason=bind_political(expected,rows,'TST',audit=audit)
            result=_request(0,'import_bind_political',step=step,entries=rows,country='TST',audit=True)
            with self.subTest(params=params):self.assertEqual(result,{'step':expected,'reason':reason,'audit':audit})
