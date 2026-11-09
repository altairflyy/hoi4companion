import copy
import unittest

from hoi4companion.overlay_diagnostics import explain_geometry


class OverlayExplanationTests(unittest.TestCase):
    def test_production_missing_unqualified_and_offscreen_remain_explicit(self):
        p=[{'key':'p','panel':'production','country':'TST','production_line_ids':[]}]
        raw={'country':'TST'}
        self.assertEqual(explain_geometry(raw,p,[])[0]['code'],'production_gui_unqualified')
        raw['production_ui']={'status':'observed','visible':True,'lines':[]}
        self.assertEqual(explain_geometry(raw,p,[])[0]['code'],'production_evidence_unknown')
        p[0]['production_missing']=True
        self.assertEqual(explain_geometry(raw,p,[])[0]['code'],'production_line_missing')
        p[0]['production_line_ids']=['1']
        raw['production_ui']['lines']=[{'line_id':'1','visible':False}]
        self.assertEqual(explain_geometry(raw,p,[])[0]['code'],'offscreen')
        p[0]['production_line_ids'].append('2')
        self.assertEqual(explain_geometry(raw,p,[])[0]['code'],'widget_ambiguous')

    def test_grouped_add_targets_and_blue_goals_are_explained_consistently(self):
        raw={'country':'TST','production_ui':{'status':'observed','visible':True}}
        p=[{'key':k,'country':'TST','panel':'production'} for k in ('a','b')]
        matches=[{'key':'a','panel':'production','navigation_stage':'add','step_keys':['a','b']}]
        result=explain_geometry(raw,p,matches)
        self.assertEqual(result[0]['step_keys'],['a','b']);self.assertIn('prodotti mancanti',result[0]['message'])
        p[0]['goal_satisfied']=True
        result=explain_geometry(raw,p[:1],[{'key':'a','panel':'production','navigation_stage':'line'}])
        self.assertIn('azzurro',result[0]['message'])

    def test_category_and_product_explanation_keeps_configuration_warning(self):
        raw={'country':'TST'}
        p=[{'key':'p','country':'TST','panel':'production','production_design_check':{'status':'different'}}]
        marks=[{'key':'p','panel':'production','navigation_stage':stage} for stage in ('add','design_candidate')]
        message=explain_geometry(raw,p,marks)[0]['message']
        self.assertIn('category',message);self.assertIn('differs',message)
        marks[1]['navigation_stage']='equipment'
        self.assertIn('exact product',explain_geometry(raw,p,marks)[0]['message'])

    def setUp(self):
        self.p=[{'key':'r','panel':'research','identity':'tools','country':'TST','folder':'industry','folder_name':'Industria'}]
        self.raw={'country':'TST','active_panel':'research','panels':{'research':{'visible':True,'folder':'engineering'}},'nodes':[]}

    def test_other_folder_has_context_and_never_reports_offscreen(self):
        before=copy.deepcopy(self.raw)
        item=explain_geometry(self.raw,self.p,[])[0]
        self.assertEqual(item['code'],'different_folder');self.assertEqual(item['folder'],'industry')
        self.assertIn('Industria',item['message']);self.assertEqual(self.raw,before)

    def test_clipped_hidden_missing_and_ambiguous_are_distinct(self):
        self.raw['panels']['research']['folder']='industry'
        self.assertEqual(explain_geometry(self.raw,self.p,[])[0]['code'],'widget_unresolved')
        node={'panel':'research','identity':'tools','folder':'industry','visible':True,'rect':[1,1,72,72],'clip':[0,0,0,0]}
        self.raw['nodes']=[node]
        self.assertEqual(explain_geometry(self.raw,self.p,[])[0]['code'],'offscreen')
        node['visible']=False;node['clip']=[0,0,72,72]
        self.assertEqual(explain_geometry(self.raw,self.p,[])[0]['code'],'widget_hidden')
        self.raw['nodes'].append(copy.deepcopy(node))
        self.assertEqual(explain_geometry(self.raw,self.p,[])[0]['code'],'widget_ambiguous')

    def test_visible_target_and_closed_panel_are_explicit(self):
        item=explain_geometry(self.raw,self.p,[{'key':'r','panel':'research'}])[0]
        self.assertEqual(item['code'],'highlighted')
        self.raw['panels']['research']['visible']=False
        self.assertEqual(explain_geometry(self.raw,self.p,[])[0]['code'],'panel_closed')

    def test_wrong_country_and_event_popup_do_not_suggest_a_research_click(self):
        self.raw['country']='OTHER'
        self.assertEqual(explain_geometry(self.raw,self.p,[])[0]['code'],'identity_mismatch')
        self.raw['country']='TST';self.raw['event_windows']=[{'visible':True}]
        self.assertEqual(explain_geometry(self.raw,self.p,[])[0]['code'],'event_foreground')

    def test_grouping_is_bounded_without_losing_targets(self):
        rows=[{**self.p[0],'key':str(i),'identity':str(i)} for i in range(30)]
        result=explain_geometry(self.raw,rows,[])
        self.assertEqual(len(result),1);self.assertEqual(len(result[0]['step_keys']),30)
