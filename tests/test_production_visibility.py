import copy
import json
from pathlib import Path
import subprocess
import unittest

from hoi4companion.campaign_calendar import calendar_date
from hoi4companion.contracts import normalise_state
from hoi4companion.campaign_conditions import timing_condition
from hoi4companion.guide_runtime import evaluate
from hoi4companion.screen_overlay import targets
from hoi4companion.production_navigation import matches
from hoi4companion.stream import state_signature
from test_production_families import CATALOG
from test_operation_import import document
from test_production_remaining import state,line
from hoi4companion.guide_importer import preview


def clock(year,month,day,hour=0):
    lengths=(31,28,31,30,31,30,31,31,30,31,30,31)
    return ((year+5000)*365+sum(lengths[:month-1])+day-1)*24+hour


def sample(tick):
    return {'usable':True,'sample':{'native_tick':tick,'country':'TST','session_id':'qualified',
        'executable_sha256':'a'*64,'country_identity_validation':'primary_selector_and_country_map'}}


class ProductionVisibilityTests(unittest.TestCase):
    def test_calendar_matches_jomini_examples_and_noleap_boundaries(self):
        self.assertEqual(calendar_date(sample(56379360)['sample']),14360101)
        self.assertEqual(calendar_date(sample(60759371)['sample']),19360101)
        self.assertEqual(calendar_date(sample(clock(1936,3,1))['sample']),19360301)
        self.assertEqual(calendar_date(sample(clock(1937,1,1)-1)['sample']),19361231)
        for tick in (True,1.2,-1,2**31,None):self.assertIsNone(calendar_date(sample(tick)['sample']))
        for field in ('session_id','executable_sha256','country_identity_validation'):
            raw=sample(clock(1938,7,1));raw['sample'].pop(field)
            self.assertNotIn('campaign.date',normalise_state(raw)['facts'])
        self.assertNotIn('campaign.date',normalise_state({**sample(clock(1938,7,1)),'usable':False})['facts'])

    def test_month_deadline_admits_rifles_only_when_provable_and_keeps_unknown_defeat_unknown(self):
        g,src=document({'category':'production','equipment_name':'Infantry Equipment','requested_factories':10})
        g=preview(g,src,CATALOG)['guide'];g['tracks']['operations'][0]['when']=timing_condition('On ROM down/no later than JUN 38')
        s=state([line('rifles','inf_1',10,10)])
        self.assertEqual(evaluate(g,s)['steps'][0]['reason'],'condition_unknown')
        s['facts']['campaign.date']={'status':'calculated','source':'fixture','value':19380629}
        self.assertEqual(evaluate(g,s)['steps'][0]['reason'],'condition_unknown')
        s['facts']['campaign.date']['value']=19380630
        r=evaluate(g,s);self.assertEqual(r['steps'][0]['status'],'completed')
        self.assertTrue(targets(g,r,CATALOG,{**s,'sample':{'country':'TST'}})[0]['goal_satisfied'])
        self.assertIsNone(timing_condition('On ROM down/no later than JUN 38 if ready'))
        self.assertIsNone(timing_condition('No later than JUNEZZ 1938'))

    def test_clock_parity_and_transport_emits_daily_changes_not_hourly_polling(self):
        values=[sample(clock(1938,6,30)),sample(clock(1938,6,30,23)),sample(clock(1938,7,1)),sample(None)]
        self.assertEqual(state_signature(values[0]),state_signature(values[1]))
        self.assertNotEqual(state_signature(values[1]),state_signature(values[2]))
        script='const c=require(process.argv[1]);let s="";process.stdin.on("data",x=>s+=x);process.stdin.on("end",()=>process.stdout.write(JSON.stringify(JSON.parse(s).map(c.state))));'
        run=subprocess.run(['node','-e',script,str(Path('src/hoi4companion/web/contracts.js').resolve())],
                           input=json.dumps(values),text=True,capture_output=True,check=True)
        self.assertEqual(json.loads(run.stdout),[normalise_state(v) for v in values])

    def test_unique_chassis_is_a_visible_design_check_never_a_completed_template(self):
        g,src=document({'category':'production','equipment_name':'Infantry Equipment','variant_name':'Custom recipe','requested_factories':10})
        g=preview(g,src,CATALOG)['guide'];s=state([line('candidate','inf_1',10,10)])
        s['facts']['production.lines']['value'][0]['name']='National model'
        r=evaluate(g,s);hud=targets(g,r,CATALOG,{**s,'sample':{'country':'TST'}})
        self.assertNotEqual(r['steps'][0]['status'],'completed');self.assertIsNone(hud[0]['goal_satisfied'])
        self.assertEqual(hud[0]['requested'],0);self.assertTrue(hud[0]['production_design_unverified'])
        control={'line_id':'candidate','equipment':'inf_1','visible':True,'rect':[0,0,100,30]}
        raw={'country':'TST','production_ui':{'status':'observed','visible':True,'lines':[control]}}
        mark=matches(raw,hud,lambda c,p:c['rect']);self.assertEqual(mark[0]['navigation_stage'],'design_candidate')
        self.assertIsNone(mark[0]['goal_satisfied'])
        raw['country']='ALT';self.assertEqual(matches(raw,hud,lambda c,p:c['rect']),[])
        s['facts']['production.lines']['value'].append(line('ambiguous','inf_1',10))
        ambiguous=targets(g,evaluate(g,s),CATALOG,{**s,'sample':{'country':'TST'}})
        self.assertNotIn('production_candidate_line_ids',ambiguous[0])
        s['facts']['production.lines']['value'][0]['name']='Custom recipe'
        exact=targets(g,evaluate(g,s),CATALOG,{**s,'sample':{'country':'TST'}})
        self.assertNotIn('production_design_unverified',exact[0]);self.assertTrue(exact[0]['goal_satisfied'])
        g['tracks']['operations'][0]['when']={'op':'fact','key':'missing','test':'eq','value':True}
        self.assertEqual(targets(g,evaluate(g,s),CATALOG,{**s,'sample':{'country':'TST'}}),[])
