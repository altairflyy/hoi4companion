import copy
import unittest
from hoi4companion.guide_runtime import evaluate
from hoi4companion.screen_overlay import targets
from test_production_remaining import guide,state,line,CATALOG


class ProductionHudTests(unittest.TestCase):
    def project(self,g,s):
        runtime=evaluate(g,s)
        raw={'usable':s['usable'],'sample':{'country':'TST'}}
        return targets(g,runtime,CATALOG,raw)

    def test_layout_projects_each_missing_target_with_requested_and_assigned_counts(self):
        g=guide();before=copy.deepcopy(g)
        s=state([line('1','sup',0),line('2','inf',10,3)])
        result=self.project(g,s)
        self.assertEqual(len(result),2)
        self.assertEqual(result[0]['production_line_ids'],['1'])
        self.assertEqual(result[1]['quantity_mode'],'remaining')
        self.assertEqual(result[1]['production_priority'],'last')
        self.assertEqual(result[1]['requested'],10)
        self.assertEqual(result[1]['assigned'],3)
        self.assertEqual(g,before)

    def test_satisfied_layout_entries_remain_tracked_and_unknown_or_waiting_stays_hidden(self):
        g=guide();s=state([line('1','sup',1),line('2','inf',10,3)])
        result=self.project(g,s)
        self.assertEqual(len(result),2)
        self.assertTrue(result[0]['goal_satisfied'])
        self.assertFalse(result[1]['goal_satisfied'])
        s['usable']=False;self.assertEqual(self.project(g,s),[])
        s=state([]);g['tracks']['operations'][0]['when']={'op':'fact','key':'not.observed','test':'eq','value':True}
        self.assertEqual(self.project(g,s),[])

    def test_completed_and_duplicate_or_missing_lines_do_not_guess_a_widget(self):
        g=guide();done=self.project(g,state([line('1','sup',1),line('2','inf',19,3)]))
        self.assertEqual(len(done),2)
        self.assertTrue(all(p['goal_satisfied'] is True for p in done))
        result=self.project(g,state([]))
        self.assertTrue(result)
        self.assertTrue(all(p['production_line_ids']==[] for p in result))

    def test_exact_quantity_does_not_treat_overallocation_as_satisfied(self):
        result=self.project(guide(),state([line('1','sup',7,7),line('2','inf',150,3)]))
        self.assertEqual([p['identity'] for p in result if p['goal_satisfied'] is not True],['sup'])

    def test_missing_products_use_catalog_interface_category_not_id_spelling_and_zero_goal_is_not_add(self):
        catalog=[{**e,'production_ui_category':'interface_category_land'} for e in CATALOG]
        g=guide();s=state([]);r=evaluate(g,s)
        result=targets(g,r,catalog,{**s,'sample':{'country':'TST'}})
        self.assertTrue(all(p['production_missing'] is True for p in result))
        self.assertTrue(all(p['production_add_control']=='infantry_button' for p in result))
        incomplete=[{**e,'production_ui_category':None} if e['id']=='inf' else e for e in catalog]
        result=targets(g,r,incomplete,{**s,'sample':{'country':'TST'}})
        self.assertIsNone(result[1]['production_add_control'])
        s=state([line('1','sup',1),line('2','inf',19,3)])
        self.assertTrue(all(p['goal_satisfied'] for p in targets(g,evaluate(g,s),catalog,{**s,'sample':{'country':'TST'}})))
        g['tracks']['operations'][0]['action']['params']['production_targets'][0]['requested_factories']=0
        from hoi4companion.operation_import import bind_operations
        bind_operations(g,catalog);empty=state([line('1','inf',20,3)])
        zero=targets(g,evaluate(g,empty),catalog,{**empty,'sample':{'country':'TST'}})[0]
        self.assertTrue(zero['production_missing']);self.assertTrue(zero['goal_satisfied'])

    def test_blue_returns_to_pending_when_allocation_is_reduced(self):
        g=guide();ready=state([line('1','sup',1),line('2','inf',150,3)])
        self.assertTrue(self.project(g,ready)[0]['goal_satisfied'])
        ready['facts']['production.lines']['value'][0]['requested_factories']=0
        self.assertFalse(self.project(g,ready)[0]['goal_satisfied'])
        self.assertTrue(self.project(g,ready)[1]['goal_satisfied'])

    def test_matching_future_quantity_never_becomes_a_blue_or_actionable_target(self):
        g=guide();g['tracks']['operations'][0]['when']={'op':'fact','key':'not.observed','test':'eq','value':True}
        self.assertEqual(self.project(g,state([line('1','sup',1),line('2','inf',19,3)])),[])
