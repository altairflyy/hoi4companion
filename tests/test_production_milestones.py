import copy,json,subprocess,tempfile,unittest
from pathlib import Path
from hoi4companion.guide import normalise_guide
from hoi4companion.guide_runtime import evaluate
from hoi4companion.guide_progress import RunStore
from hoi4companion.production_milestones import annotate,definitions
from hoi4companion.screen_overlay import targets
from test_production_remaining import guide,state,line,CATALOG


def fixture():
    g=guide();g['id']='milestones';g['dependency_policy']='explicit'
    names=['Moderate Lawyer','Economy Law','First Doctrine','MIL: Sup:1, Inf:ALL']
    raw='[Table row]\n[Column 1]\n'+'\n'.join(names)+'\n[Column 2]\nParallel research\n[Column 3]\nParallel focus'
    g['sources']=[{'id':'source','type':'file','text':raw}]
    def evidence(text):return [{'source_id':'source','basis':'explicit','quote':text}]
    rows=[]
    for key,title,fact,identity,params in [('lawyer',names[0],'politics.advisors','advisor',{'role':'political_advisor'}),
          ('law',names[1],'politics.ideas','economy',{'law_group':'economy'}),
          ('doctrine',names[2],'research.completed','doctrine',{})]:
        rows.append({'key':key,'title':title,'action':{'kind':'manual','params':{'category':'politics','instructions':[title],**params}},
                     'completion':{'op':'fact','key':fact,'test':'contains','value':identity},'evidence':evidence(title)})
    production=g['tracks']['operations'][0];production['key']='produce'
    production.pop('completion',None);production.pop('started_when',None)
    production['action']['params']['instructions']=[names[3]];production['evidence']=evidence(names[3])
    # Rebind generated production fact names after changing its source key.
    from hoi4companion.operation_import import bind_operations
    g['tracks']['operations']=rows+[production];bind_operations(g,CATALOG)
    annotate(g)
    return normalise_guide(g)


def live(*,advisor=True,law=True,doctrine=True,known=True,session='one',campaign=None):
    s=state([line('1','sup',1,1),line('2','inf',19,3)]);s['session_id']=session
    if known:
        for key,value in [('politics.advisors',['advisor'] if advisor else []),('politics.ideas',['economy'] if law else []),('research.completed',['doctrine'] if doctrine else [])]:
            s['facts'][key]={'status':'observed','source':'fixture','value':value}
    if campaign:s['facts']['campaign.id']={'status':'observed','source':'fixture','value':campaign}
    return s


class ProductionMilestoneTests(unittest.TestCase):
    def test_native_appointments_and_doctrines_restore_blue_quota_tracking(self):
        from hoi4companion.contracts import normalise_state
        g=fixture();g['tracks']['operations'][2]['completion']['key']='doctrines.selected'
        g['tracks']['operations'][-1]['action']['params'].pop('production_milestones')
        annotate(g)
        raw={'usable':True,'sample':{'country':'TST','session_id':'one','country_overview':{
            'country':'TST','politics':{'status':'partial','ideas':{'status':'observed','items':[
                {'id':'economy','group':'economy','category':'law','is_law':True}]},
                'advisors':{'status':'observed','items':[{'id':'advisor','character_id':'person','slot':0}]}},
            'doctrines':{'status':'observed','items':[{'id':'doctrine'}]}}}}
        s=live(known=False);normalized=normalise_state(raw)
        for key in ('politics.ideas','politics.advisors','doctrines.selected'):s['facts'][key]=normalized['facts'][key]
        result=evaluate(g,s)
        self.assertEqual(result['steps'][-1]['status'],'completed')
        self.assertTrue(result['steps'][-1]['production_admitted'])
        projected=targets(g,result,CATALOG,{**s,'sample':{'country':'TST'}})
        self.assertEqual(len(projected),2)
        self.assertTrue(all(p['goal_satisfied'] is True for p in projected))
        # Losing qualification must still block, rather than inventing an appointment.
        raw['sample']['country_overview']['politics']['advisors']['status']='partial'
        s['facts']['politics.advisors']=normalise_state(raw)['facts']['politics.advisors']
        result=evaluate(g,s)
        self.assertEqual(result['steps'][-1]['reason'],'milestone_unknown')
        self.assertEqual(targets(g,result,CATALOG,{**s,'sample':{'country':'TST'}}),[])

    def test_ordered_columns_attach_adoptions_not_parallel_research_and_reminders(self):
        g=fixture();p=g['tracks']['operations'][-1]['action']['params']
        self.assertEqual([m['step'] for m in p['production_milestones']],['lawyer','law','doctrine'])
        frozen=copy.deepcopy(g);self.assertEqual(annotate(g),[]);self.assertEqual(g,frozen)
        # An earlier source position never depends on a later adoption.
        g=fixture();rows=g['tracks']['operations'];rows[0]['evidence'][0]['quote']='Parallel research'
        rows[-1]['action']['params'].pop('production_milestones');annotate(g)
        self.assertNotIn('lawyer',[m['step'] for m in rows[-1]['action']['params']['production_milestones']])

    def test_counts_and_calendar_cannot_bypass_missing_adoptions(self):
        g=fixture();g['tracks']['operations'][-1]['when']={'op':'fact','key':'campaign.date','test':'gte','value':19380101}
        for field in ('advisor','law','doctrine'):
            s=live(**{field:False});s['facts']['campaign.date']={'status':'observed','source':'fixture','value':19400101}
            r=evaluate(g,s);step=r['steps'][-1]
            self.assertEqual(step['reason'],'milestone_not_completed');self.assertFalse(step['production_admitted'])
            self.assertIsNone(step['completion']['value']);self.assertTrue(step['blocked_by'])
            self.assertEqual(targets(g,r,CATALOG,{**s,'sample':{'country':'TST'}}),[])
        missing=live(known=False);missing['facts']['campaign.date']={'status':'observed','source':'fixture','value':19400101}
        unknown=evaluate(g,missing)['steps'][-1];self.assertEqual(unknown['reason'],'milestone_unknown')
        s=live();s['facts']['campaign.date']={'status':'observed','source':'fixture','value':19360101}
        self.assertEqual(evaluate(g,s)['steps'][-1]['reason'],'condition_false')
        s['facts']['campaign.date']['value']=19400101
        self.assertEqual(evaluate(g,s)['steps'][-1]['status'],'completed')

    def test_latest_law_in_same_group_replaces_earlier_order_anchor(self):
        g=fixture();rows=g['tracks']['operations'];later=copy.deepcopy(rows[1]);later['key']='newlaw';later['completion']['value']='new_economy'
        later['title']='Later Law';later['action']['params']['instructions']=['Later Law'];later['evidence'][0]['quote']='Later Law'
        g['sources'][0]['text']=g['sources'][0]['text'].replace('First Doctrine','Later Law\nFirst Doctrine')
        rows.insert(2,later);rows[-1]['action']['params'].pop('production_milestones');annotate(g)
        self.assertEqual([m['step'] for m in rows[-1]['action']['params']['production_milestones']],['lawyer','newlaw','doctrine'])

    def test_observed_history_survives_replacement_and_restart_but_not_campaign_or_rewind(self):
        a='hoi4:11111111-1111-1111-1111-111111111111:12345678';b='hoi4:22222222-2222-2222-2222-222222222222:12345678'
        g=fixture()
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/'progress.sqlite3';store=RunStore(path=path);store.create(g['id'],'one');store.bind_campaign(a,'TST','one',100)
            self.assertTrue(store.observe(g,live(campaign=a)));self.assertFalse(store.observe(g,live(campaign=a)))
            row=store.get(g['id']);self.assertEqual(len(row['campaign_conditions']['milestones']),3)
            restored=RunStore(path=path);restored.bind_campaign(a,'TST','two',101)
            replaced=live(advisor=False,law=False,session='two',campaign=a)
            self.assertEqual(evaluate(g,replaced,restored.get(g['id']))['steps'][-1]['status'],'completed')
            restored.bind_campaign(b,'TST','two',102)
            self.assertEqual(evaluate(g,live(advisor=False,session='two',campaign=b),restored.get(g['id']))['steps'][-1]['reason'],'milestone_not_completed')
            restored.bind_campaign(a,'TST','two',99)
            self.assertNotIn('campaign_conditions',restored.get(g['id']))

    def test_unknown_stale_foreign_or_manual_confirmation_never_records_achievement(self):
        g=fixture();store=RunStore();store.create(g['id'],'one');before=store.snapshot()
        for s in (live(known=False),{**live(),'usable':False},live(session='foreign')):self.assertFalse(store.observe(g,s))
        self.assertEqual(store.snapshot(),before)
        g['tracks']['operations'][-1]['depends_on']=[];g['tracks']['operations'][-1]['action']['params'].pop('production_milestones')
        self.assertEqual(annotate(g),[])

    def test_changed_condition_rejects_old_token_and_explicit_or_optional_anchors_are_preserved(self):
        g=fixture();g['tracks']['operations'][0]['completion']['value']='different_advisor'
        with self.assertRaises(ValueError):normalise_guide(g)
        for field,value in [('when',{'op':'check','key':'optional'}),('skip_when',{'op':'check','key':'skip'}),('depends_on',['law'])]:
            g=fixture();g['tracks']['operations'][0][field]=value;g['tracks']['operations'][-1]['action']['params'].pop('production_milestones');annotate(g)
            self.assertNotIn('lawyer',[m['step'] for m in g['tracks']['operations'][-1]['action']['params']['production_milestones']])

    def test_old_law_history_does_not_satisfy_changed_guide_or_another_guide(self):
        from hoi4companion.production_milestones import token_for
        g=fixture();store=RunStore();store.create(g['id'],'one');store.observe(g,live());progress=store.get(g['id'])
        g['tracks']['operations'][0]['completion']['value']='new_advisor'
        m=g['tracks']['operations'][-1]['action']['params']['production_milestones'][0]
        m['condition']=copy.deepcopy(g['tracks']['operations'][0]['completion']);m['token']=token_for('lawyer',m['condition'])
        self.assertEqual(evaluate(g,live(),progress)['steps'][-1]['reason'],'milestone_not_completed')
        g=fixture();g['id']='other'
        self.assertEqual(evaluate(g,live(advisor=False),progress)['steps'][-1]['reason'],'milestone_not_completed')

    def test_after_capture_observation_does_not_evaluate_the_same_milestones_again(self):
        from hoi4companion.backend_core import _request
        g=fixture();store=RunStore();store.create(g['id'],'one');store.observe(g,live())
        before=_request(0,'guide_cache_info')['evaluations']
        self.assertFalse(store.observe(g,live()))
        self.assertEqual(_request(0,'guide_cache_info')['evaluations'],before)

    def test_python_javascript_parity_for_current_unknown_and_historical_evidence(self):
        g=fixture();store=RunStore();store.create(g['id'],'one');store.observe(g,live());progress=store.get(g['id'])
        inputs=[{'guide':g,'state':s,'progress':p} for s,p in [(live(),None),(live(advisor=False),None),(live(known=False),None),(live(advisor=False,law=False),progress),({**live(),'usable':False},progress)]]
        script='const r=require(process.argv[1]);let s="";process.stdin.on("data",x=>s+=x);process.stdin.on("end",()=>process.stdout.write(JSON.stringify(JSON.parse(s).map(x=>r.evaluate(x.guide,x.state,x.progress)))));'
        result=subprocess.run(['node','-e',script,str(Path('src/hoi4companion/web/guide-runtime.js').resolve())],input=json.dumps(inputs),text=True,capture_output=True,check=True)
        self.assertEqual(json.loads(result.stdout),[evaluate(x['guide'],x['state'],x['progress']) for x in inputs])

    def test_web_presentation_retains_the_campaign_milestones_from_flat_progress(self):
        g=fixture();store=RunStore();store.create(g['id'],'one');store.observe(g,live())
        p=store.get(g['id']);s=live(advisor=False,law=False)
        script="const fs=require('node:fs'),p=require(process.argv[1]);const x=JSON.parse(fs.readFileSync(0,'utf8'));process.stdout.write(JSON.stringify(p.evaluate(x.guide,x.state,x.progress).model.runtime));"
        result=subprocess.run(['node','-e',script,str(Path('src/hoi4companion/web/guide-presentation.js').resolve())],input=json.dumps({'guide':g,'state':s,'progress':p}),text=True,encoding='utf-8',capture_output=True,check=True)
        self.assertEqual(json.loads(result.stdout),evaluate(g,s,p))
