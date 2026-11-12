import sys
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import numpy as np
from wsp_weights import apply_numpy_edit, apply_torch_edit, validate_edit
from semantic_eval import score
from run_model import extract_json, build_messages
from inspect_model import summarize_matrix

class WeightSpaceTests(unittest.TestCase):
    def setUp(self):
        self.weights = {'mlp.down_proj.weight': np.arange(16, dtype=np.float32).reshape(4,4)}
    def test_sparse_edit_changes_exactly_the_requested_coordinates(self):
        patch = {'schema_version':1,'max_changed_fraction':0.20,'edits':[
            {'tensor':'mlp.down_proj.weight','op':'sparse_add','entries':[
                {'index':[0,1],'delta':1.5}, {'index':[2,3],'delta':-2.0}]}]}
        revised, report = apply_numpy_edit(self.weights, patch)
        self.assertEqual(report['changed_parameters'],2)
        self.assertEqual(report['changed_fraction'], 2/16)
        self.assertEqual(float(revised['mlp.down_proj.weight'][0,1]),2.5)
        self.assertEqual(float(revised['mlp.down_proj.weight'][2,3]),9.0)
        self.assertEqual(float(self.weights['mlp.down_proj.weight'][0,1]),1.0)
        self.assertEqual(np.count_nonzero(revised['mlp.down_proj.weight'] != self.weights['mlp.down_proj.weight']),2)
    def test_rank_one_edits_are_direct_weight_writes_not_gradient_steps(self):
        patch={'schema_version':1,'max_changed_fraction':1.,'edits':[
            {'tensor':'mlp.down_proj.weight','op':'rank_one_add',
             'rows':[0,2], 'cols':[1,3], 'left':[1.0,-0.5], 'right':[2.0,3.0], 'scale':0.5}]}
        revised,report=apply_numpy_edit(self.weights,patch)
        self.assertEqual(report['changed_parameters'],4)
        self.assertAlmostEqual(float(revised['mlp.down_proj.weight'][0,1]),2.)
        self.assertAlmostEqual(float(revised['mlp.down_proj.weight'][2,3]),10.25)
    def test_unsafe_edits_are_rejected_without_mutating_source(self):
        import copy
        original = self.weights['mlp.down_proj.weight'].copy()
        cases=[
            {'schema_version':1,'edits':[{'tensor':'mlp.down_proj.weight','op':'sparse_add','entries':[{'index':[9,1],'delta':5.}]}]},
            {'schema_version':1,'edits':[{'tensor':'mlp.down_proj.weight','op':'sparse_add','entries':[{'index':[0,1],'delta':float('nan')}]}]},
            {'schema_version':1,'edits':[{'tensor':'other','op':'sparse_add','entries':[{'index':[0,1],'delta':1.}]}]},
            {'schema_version':1,'max_changed_fraction':.01,'edits':[{'tensor':'mlp.down_proj.weight','op':'sparse_add','entries':[{'index':[0,1],'delta':5.}]}]},
        ]
        for patch in cases:
            with self.subTest(patch=patch):
                with self.assertRaises(ValueError): apply_numpy_edit(self.weights,patch)
                np.testing.assert_array_equal(self.weights['mlp.down_proj.weight'],original)
    def test_weight_inspection_does_not_change_original_model(self):
        import torch
        tensor=torch.arange(12,dtype=torch.float32).reshape(3,4)
        before=tensor.clone()
        report=summarize_matrix(tensor)
        self.assertEqual(report['shape'],[3,4])
        self.assertEqual(len(report['largest_column_norms']),4)
        torch.testing.assert_close(tensor,before)
    def test_torch_direct_edit_alters_parameters_without_backward(self):
        import torch
        module=torch.nn.Linear(4,4,bias=False)
        with torch.no_grad():module.weight.fill_(1.)
        patch={'schema_version':1,'max_changed_fraction':.2,'edits':[
            {'tensor':'weight','op':'sparse_add','entries':[{'index':[1,2],'delta':.5}]}]}
        report=apply_torch_edit(module,patch)
        self.assertEqual(report['requested_positions'],1)
        self.assertAlmostEqual(module.weight[1,2].item(),1.5)
        self.assertIsNone(module.weight.grad)
    def test_prompt_requests_semantic_ops_without_guessing_game_ids(self):
        packet=build_messages('Give 25 factories to guns')
        self.assertEqual(len(packet),2)
        self.assertIn('Rust framework',packet[0]['content'])
        self.assertIn('25 factories',packet[1]['content'])
    def test_invalid_model_output_abstains(self):
        self.assertEqual(extract_json('not a JSON object')['unresolved'],['INVALID_MODEL_OUTPUT'])
        self.assertEqual(extract_json('```json\n{\"operations\":[],\"unresolved\":[]}\n```')['operations'],[])
    def test_ir_eval_counts_operations_and_does_not_award_invented_catalog_ids(self):
        expected={'operations':[{'domain':'production','operation':'allocate','target':'infantry equipment','quantity':{'mode':'total','value':25}}],'unresolved':[]}
        self.assertTrue(score(expected,expected)['exact'])
        unsafe={'operations':[{'domain':'production','operation':'allocate','target':'infantry equipment','quantity':{'mode':'total','value':25},'native_id':'inf_equipment_25'}],'unresolved':[]}
        self.assertFalse(score(expected,unsafe)['exact'])
        self.assertTrue(score(expected,unsafe)['has_extra_fields'])
    def test_ir_eval_abstentions_are_not_marked_exact(self):
        expected={'operations':[{'domain':'research','operation':'upgrade_next','target':'radio'}],'unresolved':[]}
        actual={'operations':[],'unresolved':['cannot identify research family']}
        self.assertFalse(score(expected,actual)['exact'])
        self.assertTrue(score(expected,actual)['abstained'])

if __name__=='__main__': unittest.main()
