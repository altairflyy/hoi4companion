import copy
import unittest
from unittest.mock import patch

from hoi4companion.api import Store
from hoi4companion.guide import normalise_guide


class RuntimeDefinitionCacheTests(unittest.TestCase):
    def test_equal_loaded_copies_reuse_compilation_but_edits_and_catalog_changes_invalidate(self):
        store=Store.__new__(Store);store.operation_entities=[];catalog=object()
        store.resolution_index=lambda:catalog
        guide=normalise_guide({'id':'g','dependency_policy':'explicit','sources':[{'id':'file','type':'file'}],
            'tracks':{'operations':[{'key':'note','tracking':'reminder','action':{'kind':'manual','params':{'category':'army','instructions':['Prepare']}}}]}})
        with patch('hoi4companion.informal_operations.promote_operations',return_value=True) as promote,patch('hoi4companion.operation_import.bind_operations'):
            first=store._runtime_definition(copy.deepcopy(guide))
            self.assertEqual(store._runtime_definition(copy.deepcopy(guide)),first)
            self.assertEqual(promote.call_count,1)
            guide['title']='Changed source';store._runtime_definition(copy.deepcopy(guide))
            self.assertEqual(promote.call_count,2)
            catalog=object();store._runtime_definition(copy.deepcopy(guide))
            self.assertEqual(promote.call_count,3)

    def test_resource_order_remains_part_of_compiled_definition_identity(self):
        store=Store.__new__(Store);store.operation_entities=[];index=object();store.resolution_index=lambda:index
        guide=normalise_guide({'id':'g','dependency_policy':'explicit','sources':[{'id':'file','type':'file','text':'[Table row]'}],
            'tracks':{'operations':[{'key':'note','resource_costs':{'country.experience.navy':1,'country.experience.army':2},'action':{'kind':'manual','params':{'category':'army'}}}]}})
        with patch('hoi4companion.informal_operations.promote_operations',return_value=True) as promote,patch('hoi4companion.operation_import.bind_operations'):
            store._runtime_definition(copy.deepcopy(guide))
            guide['tracks']['operations'][0]['resource_costs']={'country.experience.army':2,'country.experience.navy':1}
            result=store._runtime_definition(guide)
            self.assertEqual(promote.call_count,2)
            self.assertEqual(list(result['tracks']['operations'][0]['resource_costs']),['country.experience.army','country.experience.navy'])
