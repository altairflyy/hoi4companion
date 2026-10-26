import copy
import unittest

from hoi4companion.overlay_diagnostics import explain_geometry


class OverlayExplanationTests(unittest.TestCase):
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
