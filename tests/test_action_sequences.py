import copy
import json
import subprocess
import unittest
from hoi4companion.guide import normalise_guide
from hoi4companion.guide_runtime import evaluate
from test_guide_runtime import observed, state


def membership(key, target):
    return {'op':'fact','key':key,'test':'contains','value':target}


def sequence():
    return {'id':'sequence','tracks':{'operations':[
        {'key':'project','action':{'kind':'special_project','entity':{'kind':'project','id':'vehicles'}},
         'started_when':membership('projects.active','vehicles'), 'completion':membership('projects.completed','vehicles')},
        {'key':'expert','depends_on_started':['project'],'political_power_cost':150,
         'action':{'kind':'manual','params':{'category':'characters','advisor_id':'infantry','role':'high_command'}},
         'completion':membership('politics.advisors','infantry')},
        {'key':'monarchist','depends_on':['expert'],'political_power_cost':50,
         'action':{'kind':'manual','params':{'category':'characters','advisor_id':'monarchist','role':'political_advisor'}},
         'completion':membership('politics.advisors','monarchist')}]}}


class SequenceTests(unittest.TestCase):
    def test_shared_action_fixtures(self):
        from pathlib import Path
        for example in json.loads((Path(__file__).parent/'fixtures/action-sequences.json').read_text(encoding='utf-8')):
            with self.subTest(example=example['name']):
                result=evaluate(example['guide'],example['state'])
                self.assertEqual([s['status'] for s in result['steps']],example['statuses'])
                self.assertEqual(result['action_sequence']['political'],example['political'])
    def live(self, **extra):
        return state(**{'projects.active':[],'projects.completed':[],'politics.advisors':[], **extra})

    def test_start_dependency_releases_next_while_completion_dependency_still_waits(self):
        guide=sequence();before=copy.deepcopy(guide)
        pending=evaluate(guide,self.live())
        self.assertEqual(pending['steps'][1]['blocked_by'],['project'])
        live=self.live(**{'projects.active':['vehicles'],'country.politics.political_power':200})
        started=evaluate(guide,live)
        self.assertEqual([s['status'] for s in started['steps']],['active','ready','waiting'])
        self.assertEqual(started['action_sequence']['political'],'expert')
        self.assertEqual(started['steps'][1]['availability']['value'],None) # requirements still unknown
        guide['tracks']['operations'][1].pop('depends_on_started')
        guide['tracks']['operations'][1]['depends_on']=['project']
        self.assertEqual(evaluate(guide,live)['steps'][1]['status'],'waiting')
        self.assertEqual(before['tracks']['operations'][1]['depends_on_started'],['project'])

    def test_expensive_first_action_never_skipped_even_if_later_priority_is_higher(self):
        guide=sequence();guide['tracks']['operations'][2]['depends_on']=[]
        guide['tracks']['operations'][2]['priority']=100
        result=evaluate(guide,self.live(**{'projects.active':['vehicles'],'country.politics.political_power':70}))
        self.assertEqual(result['steps'][1]['status'],'waiting')
        self.assertEqual(result['steps'][1]['reason'],'insufficient_political_power')
        self.assertEqual(result['action_sequence']['political'],'expert')
        self.assertEqual(result['steps'][1]['action_readiness']['political_power']['available'],70)

    def test_unknown_is_reported_and_completed_membership_advances_without_cost_inference(self):
        guide=sequence();result=evaluate(guide,self.live(**{'projects.active':['vehicles']}))
        self.assertIn('political_power_unknown',result['steps'][1]['action_readiness']['unknown'])
        self.assertEqual(result['action_sequence']['political'],'expert')
        result=evaluate(guide,self.live(**{'projects.completed':['vehicles'],'politics.advisors':['infantry']}))
        self.assertEqual(result['steps'][1]['status'],'completed')
        self.assertEqual(result['action_sequence']['political'],'monarchist')

    def test_unknown_start_and_union_cycles_rejected(self):
        guide=sequence();live=self.live();del live['facts']['projects.active']
        self.assertEqual(evaluate(guide,live)['steps'][1]['status'],'unverifiable')
        guide['tracks']['operations'][0]['depends_on']=['expert']
        with self.assertRaisesRegex(ValueError,'cicli'):normalise_guide(guide)
        for patch in [{'depends_on_started':['absent']},{'political_power_cost':True},
                      {'political_power_cost':-1},{'requirements':{'op':'invalid'}}]:
            guide=sequence();guide['tracks']['operations'][1].update(patch)
            with self.assertRaises(ValueError):normalise_guide(guide)

    def test_declared_resource_requirements_and_completed_actions_are_independent(self):
        guide=sequence()
        guide['tracks']['operations'][1]['requirements']={'op':'fact','key':'country.resources.items.steel.available','test':'gte','value':5}
        live=self.live(**{'projects.active':['vehicles'],'country.politics.political_power':200})
        self.assertIsNone(evaluate(guide,live)['steps'][1]['availability']['value'])
        live['facts']['country.resources.items.steel.available']=observed(0)
        self.assertEqual(evaluate(guide,live)['steps'][1]['reason'],'action_requirements_false')
        live['facts']['country.resources.items.steel.available']=observed(5)
        self.assertTrue(evaluate(guide,live)['steps'][1]['availability']['value'])
        live['facts']['politics.advisors']=observed(['infantry'])
        live['facts']['country.politics.political_power']=observed(0)
        self.assertEqual(evaluate(guide,live)['steps'][1]['status'],'completed')
        # Spending PP or observing a click is not membership.
        live['facts']['politics.advisors']=observed([])
        self.assertNotEqual(evaluate(guide,live)['steps'][1]['status'],'completed')

    def test_full_python_javascript_parity_and_no_progress_from_foreign_run(self):
        guide=sequence();examples=[]
        for extra in [{},{'projects.active':['vehicles']},{'projects.completed':['vehicles']},
                      {'projects.active':['vehicles'],'country.politics.political_power':70},
                      {'politics.advisors':['infantry','monarchist']}]:
            examples.append({'guide':guide,'state':self.live(**extra)})
        script="const fs=require('fs'),r=require('./src/hoi4companion/web/guide-runtime.js');console.log(JSON.stringify(JSON.parse(fs.readFileSync(0,'utf8')).map(x=>r.evaluate(x.guide,x.state))));"
        expected=[evaluate(x['guide'],x['state']) for x in examples]
        result=subprocess.run(['node','-e',script],input=json.dumps(examples),capture_output=True,text=True,check=True)
        self.assertEqual(json.loads(result.stdout),expected)
        progress={'schema_version':1,'guide_id':'other','run_id':'x','started':{'project':True},'checks':{'expert':True}}
        value=evaluate(guide,self.live(),progress)
        self.assertEqual(value['progress_status'],'wrong_guide')
        self.assertNotEqual(value['steps'][1]['status'],'completed')

    def test_campaign_change_ignores_previous_order_and_membership_is_required(self):
        campaign='hoi4:00000000-0000-0000-0000-000000000001:00000001'
        live=self.live(**{'campaign.id':'hoi4:00000000-0000-0000-0000-000000000002:00000001'})
        progress={'schema_version':1,'guide_id':'sequence','run_id':'run','campaign_required':True,
                  'campaign_id':campaign,'country':'GER','checks':{'expert':True},'started':{'project':True}}
        result=evaluate(sequence(),live,progress)
        self.assertEqual(result['progress_status'],'wrong_campaign')
        self.assertEqual(result['steps'][1]['blocked_by'],['project'])
        self.assertIsNot(result['steps'][1]['completion']['value'],True)
