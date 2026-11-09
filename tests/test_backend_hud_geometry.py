"""Native geometry preserves the independent GUI/navigation contracts."""
import unittest
from copy import deepcopy
from hoi4companion.backend_core import _request
from hoi4companion.focus_ui_native import anchored_sprite_geometry, sprite_geometry
from hoi4companion.action_navigation import navigation_matches
from hoi4companion.production_navigation import matches


class NativeHudGeometryTests(unittest.TestCase):
    def test_witnessed_admission_matches_existing_country_scope_visibility_and_tab_rules(self):
        from test_overlay_geometry_provider import Reader
        from hoi4companion.overlay_geometry import NativeGeometry
        view=[99,42,0,0,400,300]
        proposals=[{'key':'f','panel':'focus','identity':'F','country':'GER','scope':'german_focus'}]
        for change in ({},{'country':'POL'},{'scope':'polish_focus'},{'pid':33},{'resolution':[800,600]},
                       {'event_windows':[{'visible':True}]},{'nodes':[]},
                       {'nodes':[{'panel':'focus','identity':'F','scope':'german_focus','visible':False}]},
                       {'nodes':[{'panel':'focus','identity':'F','scope':'german_focus','visible':True,'rect':[-100,90,80,80],'clip':[0,60,400,240]}]}):
            reader=Reader('fixture');reader.value.update(deepcopy(change))
            geometry=NativeGeometry(lambda path:{'game_dir':'unused'},reader_factory=lambda path:reader)
            expected=geometry.read(view,proposals)
            actual=_request(0,'hud_geometry_admit',raw=reader.value,proposals=proposals,view=view)
            self.assertEqual(actual,expected)

    def test_current_parent_transform_zoom_and_clip_match_existing_geometry(self):
        for scale in (.35, .75, 1., 1.5):
            parent=[scale,0,0,0,0,scale,0,0,0,0,1,0,1640,-188,0,1]
            for position in ([5,40],[-2000,-22],[39,-22]):
                args=dict(matrix=parent,position=position,size=[164,50],child_scale=.8,clip=[0,78,2560,1362])
                expected=anchored_sprite_geometry(parent,position,args['size'],.8,args['clip'])
                self.assertEqual(_request(0,'hud_sprite_geometry',**args),expected)
                args.pop('position');args.pop('child_scale');args['resolution']=[2560,1440]
                expected=sprite_geometry(parent,args['size'],args['resolution'],args['clip'])
                self.assertEqual(_request(0,'hud_sprite_geometry',**args),expected)

    def test_invalid_shear_scale_and_geometry_are_rejected(self):
        matrix=[1,0,0,0,0,1,0,0,0,0,1,0,10,-20,0,1]
        for index,value in ((0,0),(0,11),(5,2),(1,.01)):
            changed=matrix[:];changed[index]=value
            with self.assertRaises(ValueError):
                _request(0,'hud_sprite_geometry',matrix=changed,position=[0,0],size=[50,50],child_scale=1,clip=[0,0,100,100])

    def test_action_role_resource_and_facility_navigation_matches(self):
        target={'key':'law','panel':'politics','identity':'law','country':'GER','category_id':'economic_law','entity_kind':'idea','action_readiness':{'resources_available':True}}
        control={'root':'topbar','name':'player_flag','visible':True,'rect':[10,20,30,40]}
        raw={'country':'GER','action_ui':{'status':'observed','panels':dict.fromkeys(['topbar','countrypoliticsview','political_ideas_window'],False),'controls':[control]}}
        for change in ({},{'active_panel':'event'},{'country':'POL'}):
            sample={**raw,**change}
            self.assertEqual(_request(0,'hud_navigation',raw=sample,proposals=[target],passthrough=True)['actions'],navigation_matches(sample,[target],lambda c,p:c['rect']))
        denied=deepcopy(target);denied['action_readiness']['resources_available']=None
        self.assertEqual(_request(0,'hud_navigation',raw=raw,proposals=[denied],passthrough=True)['actions'],[])

    def test_production_missing_picker_and_reached_goal_match(self):
        target={'key':'p','panel':'production','identity':'inf','title':'Infantry','country':'TST','production_line_ids':['2'],'production_selector':{'equipment_ids':['inf']}}
        control={'visible':True,'rect':[1,2,30,40],'clip':[0,0,100,100],'line_id':'2','equipment':'inf'}
        raw={'country':'TST','production_ui':{'status':'observed','visible':False,'access':[control],'lines':[control]}}
        cases=[(False,{}),(True,{}),(True,{'goal_satisfied':True}),(True,{'production_missing':True,'production_add_control':'infantry_button'}),(True,{'production_design_unverified':True,'production_candidate_line_ids':['2']})]
        for visible,extra in cases:
            sample=deepcopy(raw);sample['production_ui']['visible']=visible
            sample['production_ui']['add']=[{**control,'id':'infantry_button'}]
            proposal={**target,**extra}
            expected=matches(sample,[proposal],lambda c,p:c['rect'])
            self.assertEqual(_request(0,'hud_navigation',raw=sample,proposals=[proposal],passthrough=True)['production'],expected)
