import copy
import hashlib
import json
from pathlib import Path
import subprocess
import unittest

from hoi4companion.campaign_conditions import timing_condition
from hoi4companion.guide import normalise_guide
from hoi4companion.guide_runtime import evaluate
from hoi4companion.informal_operations import promote_operations
from hoi4companion.resolution import CatalogIndex


CATALOG=CatalogIndex([
    {'kind':'focus','id':'national_a','name':'Institute Price Controls','scope':'tree','countries':['TST'],'aliases':[]},
    {'kind':'focus','id':'national_b','name':'Intervention in Spain','scope':'tree','countries':['TST'],'aliases':[]},
    {'kind':'technology','id':'tools','name':'Basic Machine Tools','aliases':[]},
    {'kind':'technology','id':'engines_3','name':'Engine III','aliases':[]},
])


def guide(timing):
    return normalise_guide({'schema_version':1,'id':'timed','title':'Fixture',
        'applies_to':{'country':'TST','tree_id':'tree'},'dependency_policy':'explicit',
        'tracks':{'operations':[{'key':'next','action':{'kind':'manual','params':{'category':'army','timing':timing}},
                                'completion':{'op':'fact','key':'order.done','test':'eq','value':True}}]}})


def state(**values):
    return {'schema_version':1,'usable':True,'facts':{k:{'status':'observed','source':'fixture','value':v}
        for k,v in {'country':'TST','focus.tree':'tree','order.done':False,**values}.items()}}


class TimingCompilerTests(unittest.TestCase):
    def test_thresholds_are_exact_units_and_never_known_from_missing_balance(self):
        for timing,key,amount in [('At 200PP','country.politics.political_power',200),
                                  ('At 60AE','country.experience.army',60),
                                  ('Once 20 NE','country.experience.navy',20),
                                  ('When 50 AFE','country.experience.air',50),
                                  ('At 12.5 army experience','country.experience.army',12.5)]:
            with self.subTest(timing=timing):
                self.assertEqual(timing_condition(timing,CATALOG,'TST'),{'op':'fact','key':key,'test':'gte','value':amount})
                g=guide(timing);promote_operations(g,CATALOG)
                self.assertEqual(evaluate(g,state())['steps'][0]['reason'],'condition_unknown')
                self.assertEqual(evaluate(g,state(**{key:amount-1}))['steps'][0]['reason'],'condition_false')
                self.assertEqual(evaluate(g,state(**{key:amount}))['steps'][0]['status'],'ready')

    def test_focus_selection_completion_and_before_are_distinct(self):
        for text,expected in [('On “Institute Price Controls”','selected'),
                              ('On Intervention in Spain focus','selected'),
                              ('On “Institute Price Controls” done','completed'),
                              ('After Institute Price Controls','completed'),
                              ('Before picking Institute Price Controls','before')]:
            g=guide(text);promote_operations(g,CATALOG)
            target='national_b' if 'Spain' in text else 'national_a'
            unknown=evaluate(g,state())['steps'][0];self.assertEqual(unknown['reason'],'condition_unknown')
            samples=[state(**{'focus.active':[],'focus.completed':[]}),state(**{'focus.active':[target],'focus.completed':[]}),
                     state(**{'focus.active':[],'focus.completed':[target]})]
            statuses=[evaluate(g,s)['steps'][0]['status']=='ready' for s in samples]
            self.assertEqual(statuses,{'selected':[False,True,True],'completed':[False,False,True],'before':[True,False,False]}[expected])

    def test_all_or_conditions_require_every_operand_to_be_understood(self):
        for timing,op in [('At 200PP and after Basic Machine Tools research','all'),
                          ('At 200PP or after Basic Machine Tools research','any')]:
            condition=timing_condition(timing,CATALOG,'TST')
            self.assertEqual(condition['op'],op)
            g=guide(timing);promote_operations(g,CATALOG)
            low=state(**{'country.politics.political_power':10,'research.completed':['tools']})
            self.assertEqual(evaluate(g,low)['steps'][0]['status']=='ready',op=='any')
        for text in ('At 200PP and when ready','At 200PP, invasion ready',
                     'When ready or after Basic Machine Tools research','At 200PP (before war)',
                     'At 20% paranoia','At free PP','On Claim','After landing in UK'):
            self.assertIsNone(timing_condition(text,CATALOG,'TST'),text)

    def test_ambiguous_kinds_foreign_scope_and_absent_names_never_choose_first(self):
        both=CatalogIndex([*CATALOG,{'kind':'technology','id':'same_name','name':'Institute Price Controls','aliases':[]}])
        self.assertIsNone(timing_condition('On Institute Price Controls',both,'TST'))
        self.assertIsNone(timing_condition('On Institute Price Controls',CATALOG,'ALT'))
        self.assertIsNone(timing_condition('On nonexistent focus',CATALOG,'TST'))
        self.assertIsNotNone(timing_condition('On Institute Price Controls focus',both,'TST'))

    def test_explicit_conditions_dependencies_and_import_idempotence_are_preserved(self):
        g=guide('At 200PP');g['tracks']['operations'][0]['when']={'op':'fact','key':'explicit.phase','test':'eq','value':True}
        previous=copy.deepcopy(g);promote_operations(g,CATALOG);self.assertEqual(g,previous)
        g=guide('After Basic Machine Tools research');promote_operations(g,CATALOG);once=copy.deepcopy(g)
        promote_operations(g,CATALOG);self.assertEqual(g,once)

    def test_existing_split_import_placeholder_is_upgraded_without_rewriting_definition(self):
        from hoi4companion.api import Store
        g=guide('At 200PP');row=g['tracks']['operations'][0]
        row['key']='word-0010--operation-2'
        token=hashlib.sha256((g['id']+'\0word-0010\0At 200PP').encode()).hexdigest()[:24]
        row['when']={'op':'fact','key':'guide.conditions.'+token,'test':'eq','value':True}
        original=copy.deepcopy(g)
        store=Store.__new__(Store);store.operation_entities=[];store.resolution_index=lambda:CATALOG
        adapted=store._runtime_definition(g)
        self.assertEqual(adapted['tracks']['operations'][0]['when']['key'],'country.politics.political_power')
        self.assertEqual(g,original)
        self.assertIs(store._runtime_definition(g),adapted)

    def test_tree_scope_and_currency_context_are_used_in_imports(self):
        catalog=CatalogIndex([*CATALOG,{'kind':'focus','id':'other_tree','name':'Institute Price Controls',
                                       'scope':'other','countries':['TST'],'aliases':[]}])
        self.assertIsNone(timing_condition('On Institute Price Controls',catalog,'TST'))
        g=guide('On Institute Price Controls');promote_operations(g,catalog)
        self.assertEqual(g['tracks']['operations'][0]['when']['args'][0]['value'],'national_a')
        g=guide('At 50AE');g['tracks']['operations'][0]['action']['params']['cost_currency']='air_experience'
        promote_operations(g,CATALOG)
        self.assertEqual(g['tracks']['operations'][0]['when']['key'],'country.experience.air')

    def test_after_war_compounds_keep_the_existing_run_scoped_history(self):
        c=timing_condition('After the war and at 200PP',CATALOG,'TST')
        self.assertEqual(c['op'],'all')
        self.assertEqual(c['args'][0]['key'],'guide.campaign.after_war')
        self.assertIsNone(timing_condition('After the war with France and at 200PP',CATALOG,'TST'))

    def test_python_javascript_runtime_outputs_match_and_stale_hides_threshold(self):
        g=guide('At 200PP and after Basic Machine Tools research');promote_operations(g,CATALOG)
        samples=[state(),state(**{'country.politics.political_power':200,'research.completed':['tools']}),
                 state(**{'country.politics.political_power':199,'research.completed':['tools']})]
        samples.append({**samples[1],'usable':False})
        result=[evaluate(g,s) for s in samples]
        module=(Path(__file__).parent.parent/'src/hoi4companion/web/guide-runtime.js').as_posix()
        script='const r=require(process.argv[1]);let s="";process.stdin.on("data",x=>s+=x);process.stdin.on("end",()=>{const p=JSON.parse(s);process.stdout.write(JSON.stringify(p.states.map(s=>r.evaluate(p.guide,s))));});'
        out=subprocess.run(['node','-e',script,module],input=json.dumps({'guide':g,'states':samples}),capture_output=True,text=True,check=True)
        self.assertEqual(json.loads(out.stdout),result)
