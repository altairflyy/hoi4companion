import copy
import unittest
from hoi4companion.production_navigation import matches

class ProductionNavigationTests(unittest.TestCase):
    def setUp(self):
        self.target={'key':'p','panel':'production','country':'TST','identity':'inf','title':'Infantry',
                     'production_line_ids':['2'],'production_selector':{'equipment_ids':['inf']}}
        self.control={'visible':True,'rect':[1,2,30,40],'clip':[0,0,100,100]}
        self.raw={'country':'TST','production_ui':{'status':'observed','visible':False,'access':[self.control],'lines':[]}}
    def run_matches(self):return matches(self.raw,[self.target],lambda c,p:c['rect'])
    def test_closed_panel_access_then_exact_line_and_no_foreign_or_duplicate_guess(self):
        self.assertEqual(self.run_matches()[0]['navigation_stage'],'access')
        self.raw['production_ui'].update(visible=True,lines=[{**self.control,'line_id':'2','equipment':'inf'}])
        self.assertEqual(self.run_matches()[0]['navigation_stage'],'line')
        self.raw['production_ui']['lines'].append(copy.deepcopy(self.raw['production_ui']['lines'][0]))
        self.assertEqual(self.run_matches(),[])
        self.raw['production_ui']['lines'].pop();self.raw['country']='ALT';self.assertEqual(self.run_matches(),[])
    def test_missing_line_stale_hidden_and_overlay_panels_fail_closed(self):
        self.raw['production_ui']['visible']=True;self.assertEqual(self.run_matches(),[])
        self.raw['production_ui']['visible']=False;self.control['visible']=False;self.assertEqual(self.run_matches(),[])
        self.control['visible']=True
        for panel in ('focus','research','event'):
            self.raw['active_panel']=panel;self.assertEqual(self.run_matches(),[])
        self.raw.pop('active_panel');self.raw['production_ui']['status']='unavailable';self.assertEqual(self.run_matches(),[])

    def test_variant_identity_survives_reorder_and_shared_access_is_drawn_once(self):
        self.assertEqual(len(matches(self.raw,[self.target,{**self.target,'key':'next'}],lambda c,p:c['rect'])),1)
        self.target['production_selector']['variant_names']=['Turán I']
        self.raw['production_ui'].update(visible=True,lines=[{**self.control,'line_id':'2','equipment':'inf','variant_name':'Turan II'}])
        self.assertEqual(self.run_matches(),[])
        self.raw['production_ui']['lines'][0]['variant_name']='TURAN I'
        self.assertEqual(self.run_matches()[0]['navigation_stage'],'line')

    def test_missing_product_routes_to_add_category_then_unique_equipment_and_never_guesses(self):
        self.target.update(production_line_ids=[],production_missing=True,production_add_control='infantry_button',goal_satisfied=False)
        ui=self.raw['production_ui'];ui.update(visible=True,add=[{**self.control,'id':'infantry_button'}])
        self.assertEqual(self.run_matches()[0]['navigation_stage'],'add')
        ui.update(picker_open=True,picker_count=1,equipment=[{**self.control,'equipment':'inf','variant_name':'Infantry'}])
        self.assertEqual([m['navigation_stage'] for m in self.run_matches()],['add','equipment'])
        ui['equipment'].append({**self.control,'equipment':'inf','variant_name':'Other'})
        self.assertEqual([m['navigation_stage'] for m in self.run_matches()],['add'])
        self.target['production_selector']['variant_names']=['Infantry']
        self.assertEqual([m['navigation_stage'] for m in self.run_matches()],['add','equipment'])
        self.target['production_missing']=None;self.assertEqual(self.run_matches(),[])

    def test_fulfilled_goals_do_not_ask_to_open_production_and_missing_categories_group_once(self):
        self.target['goal_satisfied']=True;self.assertEqual(self.run_matches(),[])
        self.raw['production_ui'].update(visible=True,lines=[{**self.control,'line_id':'2','equipment':'inf'}])
        self.assertTrue(self.run_matches()[0]['goal_satisfied'])
        self.target.update(production_line_ids=[],production_missing=True,production_add_control='infantry_button',goal_satisfied=False)
        self.raw['production_ui']['add']=[{**self.control,'id':'infantry_button'}]
        other={**self.target,'key':'sup','title':'Support','identity':'sup'}
        result=matches(self.raw,[self.target,other],lambda c,p:c['rect'])
        self.assertEqual(len(result),1)
        self.assertEqual(result[0]['production_add_names'],['Infantry','Support'])

    def test_popup_prevents_click_through_to_production_and_existing_rows(self):
        self.raw['active_panel']='event'
        self.raw['production_ui'].update(visible=True,picker_open=True,picker_count=1)
        self.assertEqual(self.run_matches(),[])

    def test_differing_recipe_highlights_both_its_category_and_current_candidate(self):
        self.target.update(production_missing=True,production_design_unverified=True,
                           production_candidate_line_ids=['2'],production_add_control='tank_button',goal_satisfied=None)
        ui=self.raw['production_ui'];ui.update(visible=True,add=[{**self.control,'id':'tank_button'}],
            lines=[{**self.control,'line_id':'2','equipment':'inf'}])
        self.assertEqual([m['navigation_stage'] for m in self.run_matches()],['add','design_candidate'])
        self.target['production_missing']=None
        self.assertEqual([m['navigation_stage'] for m in self.run_matches()],['design_candidate'])

    def test_only_factory_shortfall_or_met_quota_never_highlights_add_category(self):
        ui=self.raw['production_ui'];ui.update(visible=True,add=[{**self.control,'id':'infantry_button'}],
            lines=[{**self.control,'line_id':'2','equipment':'inf'}])
        self.target.update(production_missing=False,production_add_control='infantry_button')
        for satisfied in (False,True):
            self.target['goal_satisfied']=satisfied
            self.assertEqual([m['navigation_stage'] for m in self.run_matches()],['line'])

    def test_all_categories_and_duplicate_or_hidden_category_controls_fail_independently(self):
        self.target.update(production_line_ids=[],production_missing=True,goal_satisfied=False)
        ui=self.raw['production_ui'];ui.update(visible=True,picker_open=True,picker_count=1,
            equipment=[{**self.control,'equipment':'inf'}])
        for key in ('infantry_button','tank_button','aircraft_button','naval_button'):
            self.target['production_add_control']=key;ui['add']=[{**self.control,'id':key}]
            self.assertEqual([m['navigation_stage'] for m in self.run_matches()],['add','equipment'])
            ui['add'].append(copy.deepcopy(ui['add'][0]))
            self.assertEqual([m['navigation_stage'] for m in self.run_matches()],['equipment'])
            ui['add']=[]
        self.target['production_add_control']=None;ui['add']=[self.control]
        self.assertEqual([m['navigation_stage'] for m in self.run_matches()],['equipment'])
