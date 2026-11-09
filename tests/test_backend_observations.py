"""Native observations must keep baseline and research target decisions stable."""
import copy
import unittest
from hoi4companion.backend_core import _request
from hoi4companion.guide import normalise_guide
from hoi4companion._guide_observation_reference import RunStore
from test_guide_runtime import state


class BackendObservationTests(unittest.TestCase):
    def compare(self,guide,states,allow=True):
        guide=normalise_guide(guide)
        store=RunStore();self.addCleanup(store.close);store.create(guide['id'],'session')
        for live in states:
            before=store.get(guide['id']);snapshot=store.snapshot()
            result=_request(0,'guide_observe',guide=guide,state=live,record=before,runs=snapshot['runs'],allow_research=allow)
            changed=store.observe(guide,live,allow_research=allow)
            after=store.get(guide['id'])
            # Persisted run revisions change only when the native transaction
            # accepts the proposed observation record.
            projected=copy.deepcopy(result['record'])
            for key in ('run_revision','last_tick'):
                if key in after:projected[key]=after[key]
            self.assertEqual(result['changed'],changed)
            self.assertEqual(projected,after)

    def test_relative_building_baseline_is_not_captured_from_already_queued_order(self):
        guide={'id':'build','applies_to':{'country':'GER'},'tracks':{'operations':[{'key':'build',
            'action':{'kind':'manual','params':{'category':'construction','building_id':'arms_factory','state_id':1,'quantity_mode':'additional','quantity':2}},
            'completion':{'op':'fact','key':'guide.construction.build.added','test':'gte','value':2}}]}}
        self.compare(guide,[state(**{'country.states.items.1.military_factories':5,'construction.queued':['state:1:arms_factory']}),
                            state(**{'country.states.items.1.military_factories':5,'construction.queued':[]}),
                            state(**{'country.states.items.1.military_factories':7,'construction.queued':[]})])

    def test_progressive_research_binds_next_level_and_waits_for_earlier_step(self):
        guide={'id':'up','tracks':{'research':[{'key':'r'+str(i),'action':{'kind':'research','entity':{'kind':'technology','id':'a'}},
            'relative_research':{'query':'Guns UP','chain':['a','b','c']}} for i in range(2)]}}
        self.compare(guide,[state(**{'research.completed':['a'],'research.active':[]}),
                            state(**{'research.completed':['a'],'research.active':['b']}),
                            state(**{'research.completed':['a','b'],'research.active':[]})])

    def test_one_off_choice_and_war_milestone_keep_qualified_campaign_context(self):
        guide={'id':'choice','tracks':{'research':[{'key':'r','action':{'kind':'research','entity':{'kind':'technology','id':'c1'}},
            'research_choice':{'query':'Industry/Turbine','group':'once','mode':'once','branches':{'c':['c1','c2'],'t':['turbine']}}}],
            'operations':[{'key':'after','kind':'gate','when':{'op':'fact','key':'guide.campaign.after_war','test':'eq','value':True}}]}}
        self.compare(guide,[state(**{'research.completed':['c1'],'country.diplomacy.at_war':False}),
                            state(**{'research.completed':['c1'],'country.diplomacy.at_war':True}),
                            state(**{'research.completed':['c1','c2'],'country.diplomacy.at_war':False})])
