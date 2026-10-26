"""Guide -> runtime -> event watch -> native geometry -> HUD protocol."""
import copy
import threading
import time
import unittest

from hoi4companion.guide import normalise_guide
from hoi4companion.guide_events import compile_event_plan
from hoi4companion.guide_runtime import evaluate
from hoi4companion.overlay_bridge import Bridge
from hoi4companion.overlay_geometry import NativeGeometry
from hoi4companion.screen_overlay import targets
from test_guide_runtime import state


class EventOverlayFlowTests(unittest.TestCase):
    def setUp(self):
        self.catalog=[{'kind':'event','id':'e.1','name':'An invitation'},
            {'kind':'event_option','id':'e.1.a','event_id':'e.1','name':'Accept the invitation',
             'localized_name':'Accetta l’invito'},
            {'kind':'event_option','id':'e.1.b','event_id':'e.1','name':'Decline the invitation'}]
        self.guide=normalise_guide({'id':'guide','applies_to':{'country':'GER'},'tracks':{'operations':[
            {'key':'answer','depends_on':[], 'action':{'kind':'manual','params':{
                'event_id':'e.1','event_option_id':'e.1.a'}},
             'started_when':{'op':'fact','key':'events.visible_options','test':'contains','value':'e.1:e.1.a'}}]}})
        self.state=state(**{'events.visible':[],'events.visible_options':[]})
        self.state['sample']={'country':'GER','country_overview':{'country':'GER',
            'events':{'status':'observed','visible':[]}}}
        self.raw={'available':True,'pid':42,'country':'GER','resolution':[800,600],
            'panels':{},'events':{'status':'unknown'},'event_windows':[],'nodes':[]}
        self.requested=[]
        owner=self
        class Reader:
            def __init__(self,path):pass
            def snapshot(self,**kwargs):
                owner.requested.append(kwargs)
                return copy.deepcopy(owner.raw)
            def close(self):pass
        self.geometry=NativeGeometry(lambda _: {'game_dir':'.'},reader_factory=Reader)
        self.addCleanup(self.geometry.close)
        self.view=[99,42,0,0,800,600]

    def proposals(self,guide=None,**kwargs):
        guide=guide or self.guide
        return targets(guide,evaluate(guide,self.state),self.catalog,self.state,prepare_events=True,**kwargs)

    def open_popup(self):
        self.raw.update(active_panel='event',events={'status':'observed'},
            event_windows=[{'event_id':'e.1','instance_id':'abc','visible':True,
                            'rect':[100,100,500,400],'clip':[100,100,500,400]}],
            nodes=[{'panel':'event','event_id':'e.1','option_id':'e.1.a','identity':'e.1:e.1.a',
                    'instance_id':'abc','visible':True,'rect':[130,400,420,50],'clip':[100,100,500,400]}])

    def test_prepared_answer_uses_fresh_native_widget_without_waiting_for_live_event_update(self):
        proposed=self.proposals()
        self.assertFalse(proposed[0]['live_visible'])
        self.assertEqual(self.geometry.read(self.view,proposed)['matches'],[])
        self.open_popup()
        result=self.geometry.read(self.view,proposed)
        self.assertEqual(self.requested[-1]['event_ids'],{'e.1'})
        self.assertEqual(result['matches'][0]['identity'],'e.1:e.1.a')
        self.assertEqual(result['matches'][0]['rect'],[130,400,550,450])
        self.assertEqual(result['matches'][0]['labels'][0],'Accetta l’invito')
        self.assertEqual(result['matches'][0]['basis'],'native_gui_identity')
        self.assertNotEqual(evaluate(self.guide,self.state)['steps'][0]['status'],'completed')
        self.raw.update(event_windows=[],nodes=[],events={'status':'unknown'})
        self.assertEqual(self.geometry.read(self.view,proposed)['matches'],[])

    def test_hud_bridge_forwards_the_exact_answer_without_capture_or_ocr(self):
        self.open_popup()
        proposed=self.proposals()
        class Feed:
            guide_id='guide';selection_server='server';closed=threading.Event()
            def snapshot(self):return 1,proposed,time.monotonic(),'', 'A guide'
        bridge=Bridge(Feed(),geometry=self.geometry,sprites=object())
        result=bridge.handle({'schema_version':1,'kind':'geometry','token':2,'version':1,'view':self.view})
        self.assertTrue(result['usable'])
        self.assertTrue(result['native_available'])
        self.assertEqual(result['active_panel'],'event')
        self.assertEqual(result['matches'][0]['option_id'],'e.1.a')

    def test_wrong_guide_plan_is_recompiled_even_when_step_keys_are_reused(self):
        old=copy.deepcopy(self.guide);old['id']='old'
        old['tracks']['operations'][0]['action']['params']['event_option_id']='e.1.b'
        cached=compile_event_plan(old,self.catalog)
        before=copy.deepcopy(cached)
        proposed=self.proposals(event_plan=cached)
        self.assertEqual(proposed[0]['identity'],'e.1:e.1.a')
        self.assertEqual(cached,before)

    def test_visual_path_never_uses_an_answer_belonging_to_another_event(self):
        self.state['sample']['country_overview']['events']['visible']=[{
            'event_id':'e.1','instance_id':'abc','option_ids':['e.1.a']}]
        other=copy.deepcopy(self.catalog);other[1]['event_id']='e.2'
        self.assertEqual(targets(self.guide,evaluate(self.guide,self.state),other,self.state),[])
        self.state['sample']['country_overview']['events']['visible'][0]['option_ids']=[]
        self.assertEqual(targets(self.guide,evaluate(self.guide,self.state),self.catalog,self.state),[])

    def test_runtime_gates_exclusions_and_missing_evidence_do_not_create_watch_targets(self):
        for field,node in [('when',{'op':'fact','key':'country','test':'eq','value':'POL'}),
                           ('when',{'op':'fact','key':'missing','test':'eq','value':True}),
                           ('skip_when',{'op':'fact','key':'country','test':'eq','value':'GER'})]:
            with self.subTest(field=field,node=node):
                guide=copy.deepcopy(self.guide);guide['tracks']['operations'][0][field]=node
                self.assertEqual(self.proposals(guide),[])

    def test_guide_country_and_campaign_mismatches_hide_prepared_answers(self):
        runtime=evaluate(self.guide,self.state)
        for update in [{'guide_id':'other'},{'country':'POL'},{'mode':'preview'},
                       {'action_context':{'session_id':'other'}}]:
            self.assertEqual(targets(self.guide,runtime|update,self.catalog,self.state,prepare_events=True),[])
        self.assertEqual(targets(self.guide,runtime|{'action_context':{'campaign_id':'other'}},
                                 self.catalog,self.state,prepare_events=True),[])
        self.state['usable']=False
        self.assertEqual(self.proposals(),[])


if __name__=='__main__':unittest.main()
