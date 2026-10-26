import copy
import unittest
from hoi4companion.action_navigation import navigation_matches
from hoi4companion.screen_overlay import targets
from hoi4companion.guide_runtime import evaluate
from test_action_sequences import sequence, membership
from test_guide_runtime import state


class NavigationTests(unittest.TestCase):
    def test_access_slot_entity_are_exclusive_and_role_scoped(self):
        target={'key':'a','panel':'politics','country':'GER','identity':'infantry','entity_kind':'advisor','category_id':'high_command'}
        controls=[{'root':'topbar','name':'player_flag','visible':True,'rect':[1,2,3,4]},
                  {'root':'countrypoliticsview','name':'open_roster_button','visible':True,'category_id':'high_command','rect':[10,20,30,40]},
                  {'root':'political_ideas_window','name':'idea_entry_bg','entity_kind':'advisor','visible':True,'category_id':'high_command','entity_id':'infantry','rect':[100,200,30,40]}]
        raw={'country':'GER','action_ui':{'status':'observed','panels':dict.fromkeys(['topbar','countrypoliticsview','political_ideas_window'],False),'controls':controls}}
        admit=lambda: navigation_matches(raw,[target],lambda c,p:c['rect'])
        self.assertEqual(admit()[0]['navigation_stage'],'access')
        raw['action_ui']['panels']['countrypoliticsview']=True
        self.assertEqual(admit()[0]['navigation_stage'],'slot')
        raw['action_ui']['panels']['political_ideas_window']=True
        self.assertEqual(admit()[0]['navigation_stage'],'entity')
        controls[-1]['category_id']='political_advisor';self.assertEqual(admit(),[])
        controls[-1]['category_id']='high_command';controls.append(copy.deepcopy(controls[-1]));self.assertEqual(admit(),[])
        raw['active_panel']='event';self.assertEqual(admit(),[])

    def test_facility_compatibility_is_required_even_for_exact_project_id(self):
        target={'key':'p','panel':'projects','country':'GER','identity':'vehicles','specialization':'specialization_land'}
        control={'root':'program_window','visible':True,'entity_id':'vehicles','entity_kind':'project','specialization':'specialization_naval','rect':[1,2,3,4]}
        raw={'country':'GER','action_ui':{'status':'observed','panels':dict.fromkeys(['topbar','program_window','facilities_view','countrytechnologyview'],True),'controls':[control]}}
        self.assertEqual(navigation_matches(raw,[target],lambda c,p:c['rect']),[])
        control['specialization']='specialization_land'
        self.assertEqual(navigation_matches(raw,[target],lambda c,p:c['rect'])[0]['navigation_stage'],'entity')
        raw['country']='FRA';self.assertEqual(navigation_matches(raw,[target],lambda c,p:c['rect']),[])

    def test_runtime_sequence_is_used_by_overlay_and_guide_switch_is_rejected(self):
        guide=sequence()
        catalog=[{'kind':'project','id':'vehicles','name':'Vehicles','specialization':'specialization_land'},
                 {'kind':'advisor','id':'infantry','name':'Infantry Expert','slot':'high_command'},
                 {'kind':'advisor','id':'monarchist','name':'Monarchist','slot':'political_advisor'}]
        live=state(**{'projects.active':['vehicles'],'projects.completed':[],'politics.advisors':[]})
        raw={'usable':True,'sample':{'country':'GER','session_id':'session'}}
        runtime=evaluate(guide,live)
        proposed=targets(guide,runtime,catalog,raw)
        political=[p for p in proposed if p['panel']=='politics']
        self.assertEqual([p['key'] for p in political],['expert'])
        self.assertEqual(political[0]['identity'],'infantry')
        self.assertEqual(political[0]['category_id'],'high_command')
        self.assertEqual(targets({**guide,'id':'new'},runtime,catalog,raw),[])
        foreign=copy.deepcopy(raw);foreign['sample']['session_id']='another-session'
        self.assertEqual(targets(guide,runtime,catalog,foreign),[])
        campaign='hoi4:00000000-0000-0000-0000-000000000001:00000001'
        live['facts']['campaign.id']={'status':'observed','source':'fixture','value':campaign}
        runtime=evaluate(guide,live)
        self.assertEqual(targets(guide,runtime,catalog,raw),[]) # campaign cannot be confirmed
        raw['sample']['campaign_identity']={'status':'observed','source':'native_game_unique_id_and_seed','id':campaign}
        self.assertTrue(targets(guide,runtime,catalog,raw))
        live['facts']['country.politics.political_power']={'status':'observed','source':'fixture','value':10}
        self.assertEqual([p for p in targets(guide,evaluate(guide,live),catalog,raw) if p['panel']=='politics'],[])
