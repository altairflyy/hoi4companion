"""Static runtime preparation reuses existing guide intent, never campaign facts."""
import unittest
from copy import deepcopy
from hoi4companion.backend_core import _request
from hoi4companion.api import Store
from hoi4companion.guide import normalise_guide
from hoi4companion.resolution import catalog_index

class NativeRuntimeDefinitionTests(unittest.TestCase):
    def test_legacy_up_continuous_focus_timing_and_word_promotions_match_existing_store(self):
        catalog=[{'kind':'technology','id':'tools1','name':'Basic Machine Tools','upgrade_successors':['tools2']},
                 {'kind':'technology','id':'tools2','name':'Improved Machine Tools','upgrade_successors':[]},
                 {'kind':'focus','id':'continuous_suppress_subjects','name':'Suppress Subjects','continuous':True}]
        index=catalog_index(catalog)
        cases=[{'id':'plain','tracks':{}},
            {'id':'legacy','tracks':{'research':[{'key':'r','action':{'kind':'research','entity':{'kind':'technology','id':'tools2'}},'research_upgrade':{'query':'Tools UP','from_id':'tools1'}}]}},
            {'id':'continuous','tracks':{'focus':[{'key':'f','action':{'kind':'select_focus','entity':{'kind':'focus','id':'continuous_suppress_subjects'}}}]}},
            {'id':'word','dependency_policy':'explicit','sources':[{'id':'word','type':'file','text':'[Table row] Actions: Improve relations with Germany.'}],'tracks':{'operations':[{'key':'o','tracking':'reminder','action':{'kind':'manual','params':{'category':'diplomacy','instructions':['Improve relations with Germany.']}}}]}},
            {'id':'timing','dependency_policy':'explicit','tracks':{'operations':[{'key':'o','tracking':'reminder','action':{'kind':'manual','params':{'timing':'After 150 PP','instructions':['Prepare the front.']}}}]}}]
        for raw in cases:
            with self.subTest(guide=raw['id']):
                guide=normalise_guide(raw);before=deepcopy(guide)
                store=Store.__new__(Store);store.operation_entities=[];store.continuous_focuses={'continuous_suppress_subjects':{}};store.resolution_index=lambda:index
                expected=store._runtime_definition(guide)
                actual=_request(0,'guide_runtime_definition',guide=guide,continuous=store.continuous_focuses,library=index.reference_designs,**index.native_import_args())
                self.assertEqual(actual,expected);self.assertEqual(guide,before)
