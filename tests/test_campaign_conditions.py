import copy
import hashlib
import itertools
import json
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest

from hoi4companion.campaign_conditions import AFTER_WAR,timing_condition,uses_after_war
from hoi4companion.guide_progress import RunStore
from hoi4companion.guide_runtime import evaluate
from hoi4companion.guide import normalise_guide
from hoi4companion.informal_operations import promote_operations


def guide(identifier='postwar'):
    return normalise_guide({'schema_version':1,'id':identifier,'applies_to':{'country':'TST'},
        'dependency_policy':'explicit','tracks':{'operations':[{'key':'rebuild','when':timing_condition('after the war'),
        'completion':{'op':'fact','key':'rebuilding.done','test':'eq','value':True},
        'action':{'kind':'manual','params':{'category':'army'}}}]}})


def state(at_war,*,country='TST',session='session',usable=True,campaign=None):
    values={'country':country,'rebuilding.done':False}
    if at_war is not None:values['country.diplomacy.at_war']=at_war
    if campaign:values['campaign.id']=campaign
    return {'schema_version':1,'usable':usable,'session_id':session,'facts':{
        key:{'status':'observed','source':'fixture','value':value} for key,value in values.items()}}


class CampaignConditionTests(unittest.TestCase):
    def store(self,path=None):
        ids=itertools.count(1)
        return RunStore(id_factory=lambda:'run-'+str(next(ids)),path=path)

    def test_complete_generic_phrases_only_and_explicit_predicates_are_preserved(self):
        for text in ('After the war','after war','Once the war is over','When war is over','Dopo la guerra'):
            self.assertEqual(timing_condition(text),{'op':'fact','key':AFTER_WAR,'test':'eq','value':True})
        for text in ('After the war with France','after the civil war','Before the war','After the war; before event'):
            self.assertIsNone(timing_condition(text))
        value=guide()
        row=value['tracks']['operations'][0]
        row['action']['params']['timing']='After the war'
        row['when']={'op':'fact','key':'authored.custom','test':'eq','value':True}
        promote_operations(value,[])
        self.assertEqual(row['when']['key'],'authored.custom')
        row.pop('when')
        promote_operations(value,[])
        self.assertEqual(row['when']['key'],AFTER_WAR)
        text='After the war'
        token=hashlib.sha256((value['id']+'\0'+row['key']+'\0'+text).encode('utf8')).hexdigest()[:24]
        row['when']={'op':'fact','key':'guide.conditions.'+token,'test':'eq','value':True}
        promote_operations(value,[])
        self.assertEqual(row['when']['key'],AFTER_WAR)
        row.pop('when');row['tracking']='reminder';row['action']['params']={'instructions':['After the war: War Economy (150PP)']}
        catalog=[{'kind':'law','id':'war_economy','name':'War Economy','group':'economic_laws','aliases':[]}]
        promote_operations(value,catalog)
        promoted=value['tracks']['operations'][0]
        self.assertEqual(promoted['when']['key'],AFTER_WAR)
        self.assertEqual(promoted['action']['params']['law_id'],'war_economy')

    def test_initial_peace_does_not_satisfy_after_war_and_unknown_never_unlocks(self):
        value=guide();store=self.store();store.create(value['id'],'session')
        initial=store.get(value['id'])
        self.assertFalse(store.observe(value,state(False)))
        self.assertEqual(evaluate(value,state(False),initial)['steps'][0]['reason'],'condition_false')
        self.assertTrue(store.observe(value,state(True)))
        self.assertFalse(store.observe(value,state(True)))
        observed=store.get(value['id'])
        self.assertEqual(observed['campaign_conditions'],{'country':'TST','war_seen':True})
        self.assertEqual(evaluate(value,state(True),observed)['steps'][0]['reason'],'condition_false')
        self.assertEqual(evaluate(value,state(False),observed)['steps'][0]['status'],'ready')
        self.assertEqual(evaluate(value,state(None),observed)['steps'][0]['reason'],'condition_unknown')
        self.assertEqual(evaluate(value,state(False,usable=False),observed)['steps'][0]['reason'],'condition_unknown')
        self.assertEqual(initial.get('campaign_conditions',{}),{})

    def test_existing_structured_guide_timing_is_adapted_once_without_rewriting(self):
        from hoi4companion.api import Store
        from hoi4companion.resolution import CatalogIndex
        store=Store.__new__(Store);store.operation_entities=[]
        index=CatalogIndex([]);store.resolution_index=lambda:index
        original=guide();row=original['tracks']['operations'][0]
        row.pop('when');row['action']['params']['timing']='After the war'
        before=copy.deepcopy(original)
        adapted=store._runtime_definition(original)
        self.assertEqual(adapted['tracks']['operations'][0]['when']['key'],AFTER_WAR)
        self.assertIs(store._runtime_definition(original),adapted)
        self.assertEqual(original,before)
        row['action']['params']['timing']='After the war with France'
        self.assertIs(store._runtime_definition(original),original)
        row['action']['params']['timing']='After the war';original['dependency_policy']='legacy'
        self.assertIs(store._runtime_definition(original),original)

    def test_stale_foreign_session_and_other_country_cannot_record_a_war(self):
        value=guide();store=self.store();store.create(value['id'],'session')
        before=store.snapshot()
        for sample in (state(True,usable=False),state(True,country='ALT'),state(True,session='foreign'),state(None)):
            self.assertFalse(store.observe(value,sample))
        self.assertEqual(store.snapshot(),before)

    def test_guide_run_campaign_and_timeline_changes_do_not_reuse_other_history(self):
        a='hoi4:11111111-1111-1111-1111-111111111111:12345678'
        b='hoi4:22222222-2222-2222-2222-222222222222:12345678'
        value=guide();other=guide('other');store=self.store();store.create(value['id'],'session');store.create(other['id'],'session')
        store.bind_campaign(a,'TST','session',20)
        self.assertTrue(store.observe(value,state(True,campaign=a)))
        self.assertNotIn('campaign_conditions',store.get(other['id']))
        store.bind_campaign(b,'TST','session',30)
        self.assertNotIn('campaign_conditions',store.get(value['id']))
        store.bind_campaign(a,'TST','session',21)
        self.assertEqual(evaluate(value,state(False,campaign=a),store.get(value['id']))['steps'][0]['status'],'ready')
        store.bind_campaign(a,'TST','session',10)
        self.assertNotIn('campaign_conditions',store.get(value['id']))
        self.assertEqual(evaluate(value,state(False,campaign=a),store.get(value['id']))['steps'][0]['reason'],'condition_false')
        store.observe(value,state(True,campaign=a));store.create(value['id'],'session')
        self.assertNotIn('campaign_conditions',store.get(value['id']))

    def test_transaction_failure_rolls_back_and_campaign_history_survives_restart(self):
        campaign='hoi4:11111111-1111-1111-1111-111111111111:12345678'
        value=guide()
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/'progress.sqlite3';store=self.store(path)
            store.create(value['id'],'session');store.bind_campaign(campaign,'TST','session',20)
            store.observe(value,state(True,campaign=campaign))
            restored=RunStore(path=path)
            restored.bind_campaign(campaign,'TST','another-session',21)
            self.assertEqual(evaluate(value,state(False,session='another-session',campaign=campaign),restored.get(value['id']))['steps'][0]['status'],'ready')
        store=self.store();store.create(value['id'],'session');before=store.snapshot()
        def fail(_):raise OSError('storage failure')
        store._database=SimpleNamespace(save=fail)
        with self.assertRaisesRegex(OSError,'storage failure'):store.observe(value,state(True))
        self.assertEqual(store.snapshot(),before)

    def test_python_javascript_evaluate_the_same_condition_and_reject_malformed_history(self):
        value=guide();store=self.store();store.create(value['id'],'session');initial=store.get(value['id'])
        store.observe(value,state(True));observed=store.get(value['id'])
        examples=[{'guide':value,'state':sample,'progress':progress} for sample in
                  (state(False),state(True),state(None),state(False,usable=False),state(False,country='ALT'),state(False,session='other'))
                  for progress in (initial,observed,None)]
        module=(Path(__file__).parent.parent/'src/hoi4companion/web/guide-runtime.js').as_posix()
        code='const r=require(process.argv[1]);let s="";process.stdin.on("data",x=>s+=x);process.stdin.on("end",()=>process.stdout.write(JSON.stringify(JSON.parse(s).map(x=>r.evaluate(x.guide,x.state,x.progress)))));'
        result=subprocess.run(['node','-e',code,module],input=json.dumps(examples),text=True,capture_output=True,check=True)
        self.assertEqual(json.loads(result.stdout),[evaluate(x['guide'],x['state'],x['progress']) for x in examples])
        malformed=[]
        for history in (None,True,[],{'country':'TST','war_seen':1},{'country':'TST','war_seen':True,'extra':True},{'country':'long','war_seen':True}):
            altered=copy.deepcopy(observed);altered['campaign_conditions']=history
            with self.assertRaises(ValueError):evaluate(value,state(False),altered)
            malformed.append({'guide':value,'state':state(False),'progress':altered})
        reject=code.replace('r.evaluate(x.guide,x.state,x.progress)','(()=>{try{r.evaluate(x.guide,x.state,x.progress);return false;}catch{return true;}})()')
        rejected=subprocess.run(['node','-e',reject,module],input=json.dumps(malformed),text=True,capture_output=True,check=True)
        self.assertEqual(json.loads(rejected.stdout),[True]*len(malformed))
        self.assertTrue(uses_after_war(value))
        value['tracks']['operations'][0].pop('when')
        self.assertFalse(uses_after_war(value))
