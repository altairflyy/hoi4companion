import copy
import unittest
from hoi4companion.screen_overlay import targets,rectangles


def line(text,x=20,y=100,width=200,height=18):
    return {'text':text,'words':[{'text':text,'x':x,'y':y,'width':width,'height':height}]}


class ScreenOverlayTests(unittest.TestCase):
    def setUp(self):
        self.guide={'id':'universal','applies_to':{'country':'TST'},'tracks':{'focus':[{'key':'f','action':{'kind':'select_focus','entity':{'kind':'focus','id':'focus','scope':'tree'}}}]}}
        self.runtime={'guide_id':'universal','mode':'live','country':'TST','parallel':{'focus':{'steps':['f']}},'steps':[{'key':'f','status':'ready'}]}
        self.state={'usable':True,'sample':{'country':'TST'}}
        self.catalog=[{'kind':'focus','id':'focus','scope':'tree','name':'Develop Industry','localized_name':'Sviluppare l’industria'}]
    def test_only_runtime_admitted_nodes_and_exact_country_are_proposed(self):
        guide=copy.deepcopy(self.guide);result=targets(self.guide,self.runtime,self.catalog,self.state)
        self.assertEqual(result[0]['key'],'f');self.assertIn('Sviluppare l’industria',result[0]['labels']);self.assertEqual(self.guide,guide)
        for update in [{'mode':'preview'},{'country':None},{'country':'OTHER'},{'guide_id':'other'},{'parallel':{}}]:
            self.assertEqual(targets(self.guide,self.runtime|update,self.catalog,self.state),[])
        self.assertEqual(targets(self.guide,self.runtime,self.catalog,{'usable':False}),[])
        self.assertEqual(targets(self.guide,self.runtime,self.catalog,{'usable':True,'sample':{'country':'OTHER'}}),[])
        self.assertEqual(targets(self.guide,self.runtime,self.catalog,{'usable':True,'sample':{}}),[])
        self.guide['applies_to']={}
        self.assertEqual(targets(self.guide,self.runtime,self.catalog,self.state)[0]['key'],'f')
    def test_exact_multiline_label_is_unique_and_requires_panel(self):
        proposed=targets(self.guide,self.runtime,self.catalog,self.state)
        image={'width':800,'height':600,'lines':[line('Focus',y=10),line('nazionali Test',x=250,y=10),line('Sviluppare',width=110),line('l’industria',y=120,width=110)]}
        matches=rectangles(image,proposed);self.assertEqual(len(matches),1);self.assertEqual(matches[0]['basis'],'exact_unique_ocr')
        self.assertEqual(rectangles(image|{'lines':image['lines'][2:]},proposed),[])
        self.assertEqual(rectangles(image|{'lines':image['lines']+[line('Sviluppare l’industria',x=400,y=300)]},proposed),[])
        self.assertEqual(rectangles(image|{'lines':[line('Focus nazionali',y=10),line('Sviluppare l’industria',x=float('nan'))]},proposed),[])
        self.assertEqual(rectangles(image,proposed+[proposed[0]|{'identity':'other'}]),[])
    def test_events_need_observed_window_and_localized_event_and_choice(self):
        self.guide['tracks']={'operations':[{'key':'event','action':{'kind':'manual','params':{'category':'politics','event_id':'evt.1','event_option_id':'evt.1.a','event_name':'An Event'}}}]}
        self.runtime['steps']=[{'key':'event','status':'active'}]
        self.catalog=[{'kind':'event','id':'evt.1','name':'An Event','localized_name':'Un evento'},{'kind':'event_option','id':'evt.1.a','name':'Accept the proposal','localized_name':'Accetta la proposta'}]
        self.assertEqual(targets(self.guide,self.runtime,self.catalog,self.state),[])
        self.state['sample']={'country':'TST','country_overview':{'country':'TST','events':{'status':'observed','visible':[{'event_id':'evt.1'}]}}}
        proposed=targets(self.guide,self.runtime,self.catalog,self.state);self.assertEqual(len(proposed),1)
        image={'width':800,'height':600,'lines':[line('Un evento',y=20),line('Accetta la proposta')]}
        self.assertEqual(len(rectangles(image,proposed)),1)
        self.assertEqual(rectangles(image|{'lines':[line('Accetta la proposta')]},proposed),[])
        self.state['sample']['country_overview']['country']='OTHER'
        self.assertEqual(targets(self.guide,self.runtime,self.catalog,self.state),[])
    def test_laws_advisors_and_production_use_resolved_ids_not_prose(self):
        self.guide['tracks']={'operations':[
            {'key':'law','action':{'kind':'manual','params':{'category':'politics','law_id':'law','law_group':'economy'}}},
            {'key':'advisor','action':{'kind':'manual','params':{'category':'characters','advisor_id':'advisor','person_id':'person','role':'chief'}}},
            {'key':'production','action':{'kind':'manual','params':{'category':'production','variant_name':'Custom Model','production_selector':{'equipment_ids':['equipment']},'assigned_factories':5}}},
            {'key':'future','action':{'kind':'manual','params':{'category':'politics','law_id':'later','law_group':'economy'}}}]}
        self.runtime['steps']=[{'key':key,'status':'unverifiable','reason':'manual_confirmation_required'} for key in ('law','advisor','production','future')]
        self.catalog=[{'kind':'law','id':'law','name':'War Economy'},{'kind':'law','id':'later','name':'Other Law'},{'kind':'advisor','id':'advisor','name':'Example Person'}]
        result=targets(self.guide,self.runtime,self.catalog,self.state)
        self.assertEqual([r['key'] for r in result],['law','advisor','production']);self.assertEqual(result[-1]['quantity'],5)
        self.runtime['steps'][0]['reason']='condition_unknown'
        self.assertNotIn('law',[r['key'] for r in targets(self.guide,self.runtime,self.catalog,self.state)])

    def test_production_reuses_runtime_qualified_models_and_aggregate_target(self):
        self.guide['tracks']={'operations':[{'key':'p','action':{'kind':'manual','params':{
            'category':'production','equipment_id':'family','assigned_factories':2,
            'quantity_mode':'additional','production_selector':{'equipment_ids':['model']}}}}]}
        self.runtime['steps']=[{'key':'p','status':'active','production':{
            'target':7,'lines':[{'name':'Actual Local Model','equipment':'model'}]}}]
        result=targets(self.guide,self.runtime,[],self.state)
        self.assertEqual(result[0]['labels'],['Actual Local Model'])
        self.assertEqual(result[0]['quantity'],7)
        self.runtime['steps'][0]['status']='blocked'
        self.assertEqual(targets(self.guide,self.runtime,[],self.state),[])

    def test_research_requires_exact_visible_label_and_screen_context(self):
        proposal={'key':'r','panel':'research','identity':'tech','labels':['Macchine utensili di base'],'status':'ready'}
        frame={'width':800,'height':600,'lines':[line('Ricerca',y=10),line('Macchine utensili di base')]}
        self.assertEqual(len(rectangles(frame,[proposal])),1)
        self.assertEqual(rectangles(frame|{'lines':[line('Ricerca',y=10),line('Macchine utensili migliorate')]},[proposal]),[])
        # Icon-only nodes provide no text evidence and must not acquire guessed boxes.
        self.assertEqual(rectangles(frame|{'lines':[line('Ricerca',y=10)]},[proposal]),[])

    def test_research_projection_keeps_slot_choices_across_folders_in_priority_order(self):
        self.guide['tracks']={'research':[{'key':str(i),'folder':folder,'folder_name':folder,
            'action':{'kind':'research','entity':{'kind':'technology','id':'tech'+str(i)}}}
            for i,folder in enumerate(('industry','engineering','industry','infantry'))]}
        self.catalog=[{'kind':'technology','id':'tech'+str(i),'name':'Technology '+str(i)} for i in range(4)]
        self.runtime['steps']=[{'key':str(i),'status':'ready'} for i in range(4)]
        for keys in (['1','0','2'],['3'],[]):
            self.runtime['parallel']={'research':{'capacity':len(keys),'steps':keys}}
            result=targets(self.guide,self.runtime,self.catalog,self.state)
            self.assertEqual([r['key'] for r in result],keys)
            self.assertEqual(len(result),len(keys))
        self.runtime['steps'][1]['status']='active'
        self.runtime['parallel']={'research':{'capacity':1,'steps':['0']}}
        self.assertEqual([r['key'] for r in targets(self.guide,self.runtime,self.catalog,self.state)],['0'])


if __name__=='__main__':unittest.main()
