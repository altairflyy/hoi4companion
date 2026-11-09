"""Qualified foreign interactions; fixtures are not visual game verification."""
import copy
import json
from pathlib import Path
import subprocess
import unittest

from hoi4companion.contracts import normalise_state
from hoi4companion.country import CountryReader
from hoi4companion.live import LiveReadError, compile_watch_reads, watch_changed
from test_native_fixture import Fixture


def wire(interactions=None, missions=None):
    return {'usable':True,'sample':{'country':'ROM','country_overview':{'country':'ROM',
        'diplomacy':{'status':'observed','at_war':False,'interactions':interactions or
            {'status':'observed','countries':['ROM','ITA'],'improving_relations':['ITA'],'non_aggression_pacts':[]}},
        'intelligence':{'status':'observed','is_created':True,'in_creation':False,'missions':missions or
            {'status':'observed','countries':['ROM','ITA'],'items':[{'id':'agent-1','mission_id':'diplomatic_pressure','target_country':'ITA','state':'active'}]}}}}}


class ForeignInteractionTests(unittest.TestCase):
    def test_qualified_membership_and_python_javascript_parity(self):
        cases=[wire()]
        for field,bad in [('diplomacy',{'status':'partial'}),('intelligence',{'status':'unavailable'})]:
            x=wire();x['sample']['country_overview'][field]=bad;cases.append(x)
        x=wire();x['usable']=False;cases.append(x)
        x=wire();x['sample']['country_overview']['country']='HUN';cases.append(x)
        for mutate in ('duplicate','unknown_target','bad_state','unsupported_mission','missing_countries'):
            x=wire();m=x['sample']['country_overview']['intelligence']['missions']
            if mutate=='duplicate':m['items']*=2
            elif mutate=='unknown_target':m['items'][0]['target_country']='ZZZ'
            elif mutate=='bad_state':m['items'][0]['state']='probably_active'
            elif mutate=='unsupported_mission':m['items'][0]['mission_id']='do_anything'
            else:m.pop('countries')
            cases.append(x)
        x=wire();x['sample']['country_overview']['intelligence']['missions']['items'][0]['state']='captured';cases.append(x)
        x=wire();x['sample']['country_overview']['diplomacy']['interactions']['improving_relations']=['ITA','ITA'];cases.append(x)
        x=wire();m=x['sample']['country_overview']['intelligence']['missions'];m['countries']=['ROM','ITA\n'];m['items'][0]['target_country']='ITA\n';cases.append(x)
        py=[normalise_state(x) for x in cases]
        self.assertEqual(py[0]['facts']['diplomacy.improving_relations']['value'],['ITA'])
        self.assertEqual(py[0]['facts']['intelligence.active_missions']['value'],['diplomatic_pressure:ITA'])
        for n in range(5,10):self.assertEqual(py[n]['facts']['intelligence.active_missions']['status'],'unknown')
        self.assertEqual(py[10]['facts']['intelligence.active_missions']['value'],[])
        self.assertEqual(py[11]['facts']['diplomacy.improving_relations']['status'],'unknown')
        self.assertFalse(any(k.startswith('country.intelligence.missions.') for k in py[5]['facts']))
        module=str(Path('src/hoi4companion/web/contracts.js').resolve())
        code='const c=require(process.argv[1]);let s="";process.stdin.on("data",x=>s+=x);process.stdin.on("end",()=>process.stdout.write(JSON.stringify(JSON.parse(s).map(c.state))));'
        js=json.loads(subprocess.run(['node','-e',code,module],input=json.dumps(cases),text=True,capture_output=True,check=True).stdout)
        keys=('diplomacy.countries','diplomacy.improving_relations','diplomacy.non_aggression_pacts','intelligence.active_missions')
        for a,b in zip(py,js):
            for key in keys:
                self.assertEqual(a['facts'].get(key,{}).get('value'),b['facts'].get(key,{}).get('value'))
                self.assertEqual(a['facts'].get(key,{}).get('status'),b['facts'].get(key,{}).get('status'))

    def test_generic_bindings_do_not_replace_authored_conditions(self):
        from hoi4companion.foreign_interactions import bind
        from hoi4companion.resolution import CatalogIndex
        catalog=CatalogIndex([],country_names={'ITA':['Italy'],'ROM':['Romania']})
        for task,field in [('improve relations','diplomacy.improving_relations'),('non-aggression pact','diplomacy.non_aggression_pacts')]:
            row={'key':'a','action':{'kind':'manual','params':{'category':'politics','task':task,'target_country':'Italy'}}}
            self.assertTrue(bind(row,catalog))
            self.assertEqual(row['action']['params']['target_country'],'ITA')
            self.assertEqual(row['completion']['args'][0]['args'][-1]['key'],field)
            row['completion']={'op':'check','key':'custom'};bind(row,catalog)
            self.assertEqual(row['completion'],{'op':'check','key':'custom'})

    def test_compound_abbreviations_preserve_sequence_and_unknown_words(self):
        from hoi4companion.foreign_interactions import commands
        from hoi4companion.resolution import CatalogIndex
        catalog=CatalogIndex([],country_names={'CZE':['Czechoslovakia'],'ROM':['Romania']})
        values=commands('REL + NA CZE, cancel REL, REL+spy pressure ROM',catalog)
        self.assertEqual([(r['params']['operation_id'],r['params']['target_country']) for r in values],
            [('improve_relations','CZE'),('non_aggression_pact','CZE'),('stop_improving_relations','CZE'),('improve_relations','ROM'),('assign_operative_mission','ROM')])
        self.assertEqual(values[0]['after_previous'],False)
        self.assertFalse(values[1]['after_previous'])
        self.assertEqual(values[2]['after_keys'],[0,1])
        self.assertEqual(values[3]['after_keys'],[2])
        self.assertEqual(values[4]['after_keys'],[2])
        self.assertIsNone(commands('improve relations with Italy if useful',catalog))
        self.assertIsNone(commands('spy pressure ROM and invade',catalog))

    def test_run_history_preserves_execution_across_stop_and_respects_dependencies(self):
        from hoi4companion.foreign_interactions import bind,token,predicate
        from hoi4companion.resolution import CatalogIndex
        from hoi4companion.guide_progress import RunStore
        from hoi4companion.guide_runtime import evaluate
        catalog=CatalogIndex([])
        rows=[{'key':key,'action':{'kind':'manual','params':{'operation_id':op,'target_country':'ITA'}}}
              for key,op in [('rel','improve_relations'),('pact','non_aggression_pact'),('stop','stop_improving_relations')]]
        rows[2]['depends_on']=['rel','pact']
        for row in rows:bind(row,catalog)
        guide={'schema_version':1,'id':'foreign','title':'Foreign','dependency_policy':'explicit','applies_to':{'country':'ROM'},'tracks':{'operations':rows}}
        store=RunStore();store.create('foreign','foreign-session')
        state=wire();state['sample']['session_id']='foreign-session'
        self.assertTrue(store.observe(guide,state))
        self.assertNotIn(token(rows[2],predicate(rows[2]['action']['params'])),store.get('foreign')['campaign_conditions']['milestones'])
        result=evaluate(guide,state,store.get('foreign'))
        self.assertEqual(result['steps'][2]['status'],'waiting')
        state['sample']['country_overview']['diplomacy']['interactions']['non_aggression_pacts']=['ITA']
        store.observe(guide,state)
        state['sample']['country_overview']['diplomacy']['interactions']['improving_relations']=[]
        store.observe(guide,state)
        result=evaluate(guide,state,store.get('foreign'))
        self.assertEqual([r['status'] for r in result['steps']],['completed']*3)
        state['sample']['session_id']='another-session'
        self.assertNotEqual(evaluate(guide,state,store.get('foreign'))['progress_status'],'accepted')

    def test_source_promotion_keeps_existing_substep_keys_and_is_idempotent(self):
        from hoi4companion.informal_operations import promote_operations
        from hoi4companion.operation_import import bind_operations
        from hoi4companion.resolution import CatalogIndex
        catalog=CatalogIndex([])
        rows=[{'key':'word-0001','tracking':'reminder','action':{'kind':'manual','params':{'instructions':['REL + NA CZE, cancel REL, REL+spy pressure ROM'],'category':'checkpoints'}}},
              {'key':'word-0001--operation-2','tracking':'reminder','action':{'kind':'manual','params':{'instructions':['Unrelated method note'],'category':'checkpoints'}}}]
        guide={'id':'source','applies_to':{'country':'HUN'},'dependency_policy':'explicit','tracks':{'operations':rows}}
        promote_operations(guide,catalog);bind_operations(guide,catalog)
        self.assertEqual(len(rows),6)
        self.assertEqual(rows[-1]['key'],'word-0001--operation-2')
        self.assertEqual(rows[2]['depends_on'],[rows[0]['key'],rows[1]['key']])
        old=copy.deepcopy(guide);promote_operations(guide,catalog);bind_operations(guide,catalog)
        self.assertEqual(guide,old)

    def test_quantified_network_goal_is_not_completed_by_assignment(self):
        from hoi4companion.foreign_interactions import bind
        from hoi4companion.resolution import CatalogIndex
        row={'key':'network','action':{'kind':'manual','params':{'category':'projects','operation_name':'build network','target_country':'ITA','network_strength':50}}}
        self.assertFalse(bind(row,CatalogIndex([])))
        self.assertNotIn('completion',row)

    def test_standalone_focus_heading_gates_every_diplomatic_child(self):
        from hoi4companion.informal_operations import promote_operations
        from hoi4companion.operation_import import bind_operations
        from hoi4companion.resolution import CatalogIndex
        catalog=CatalogIndex([{'kind':'focus','id':'test_reach_out','name':'Reach Out to Candidates'}])
        guide={'id':'source','applies_to':{'country':'HUN'},'dependency_policy':'explicit','tracks':{
            'focus':[{'key':'focus','action':{'kind':'select_focus','entity':{'kind':'focus','id':'test_reach_out'}}}],
            'operations':[{'key':'order','tracking':'reminder','action':{'kind':'manual','params':{'instructions':['On Reach Out…\n- REL + NA CZE'],'category':'checkpoints'}}}]}}
        promote_operations(guide,catalog);bind_operations(guide,catalog)
        rows=guide['tracks']['operations'];self.assertEqual(len(rows),2)
        for row in rows:self.assertIn('when',row)
        self.assertEqual(rows[0]['when'],rows[1]['when'])


class ForeignNativeTests(unittest.TestCase):
    def fixture(self):
        f=Fixture();f.profile.foreign_interactions_layout={'version':1,'signatures':()}
        f.profile.bytes=lambda address,size:b''
        other=f.object('CCountry');f.put(other+8,'<i',2)
        f.array(f.world+0x310,[f.country,other]);f.array(f.world+0x340,[0,0,1],4)
        tags=f.allocate();f.string(tags+32,'ROM');f.string(tags+64,'ITA')
        f.put(f.world+0x358,'<Q',tags);f.put(f.world+0x364,'<i',3)
        dip=f.object('CDiplomacyStatus');f.put(f.country+0xF88,'<Q',dip);f.put(dip+0x2D0,'<Q',f.country)
        f.array(dip+0x20,[])
        agency=f.object('CIntelligenceAgency');f.put(f.country+0xFC0,'<Q',agency);f.put(agency+0x78,'<Q',f.country)
        f.put(agency+0xC0,'<BB',1,0);f.array(agency+0xF0,[])
        return f,dip,agency

    def test_relations_are_directional_pacts_bilateral_and_cancellation_watched(self):
        from hoi4companion.foreign_interactions_native import read_relations
        f,dip,_=self.fixture();improve=f.object('CImproveRelationsRelation');pact=f.object('CNonAggressionPactRelation')
        for p,t,left,right in ((improve,0x2CD7,1,2),(pact,0x2FBC,2,1)):
            f.put(p+8,'<i',t);f.put(p+0x10,'<ii',left,right);f.put(p+0x48,'<B',0)
        f.array(dip+0x20,[improve,pact]);r=CountryReader(f,f.world,f.country,1)
        result=read_relations(r,dip);r.s.verify()
        self.assertEqual(result['improving_relations'],['ITA']);self.assertEqual(result['non_aggression_pacts'],['ITA'])
        plan=compile_watch_reads(r.watch);f.put(improve+0x48,'<B',1);self.assertTrue(watch_changed(f,plan))
        self.assertEqual(read_relations(CountryReader(f,f.world,f.country,1),dip)['improving_relations'],[])
        f.put(improve+0x48,'<B',0);f.put(improve+0x10,'<ii',2,1)
        self.assertEqual(read_relations(CountryReader(f,f.world,f.country,1),dip)['improving_relations'],[])
        f.put(pact+0x10,'<ii',2,2)
        with self.assertRaises(LiveReadError):read_relations(CountryReader(f,f.world,f.country,1),dip)

    def test_missions_require_owned_agency_matching_agent_type_and_active_state(self):
        from hoi4companion.foreign_interactions_native import read_intelligence
        f,_,agency=self.fixture();agent=f.object('COperativeLeader');f.put(agent+8,'<II',0x1269,1)
        f.put(agent+0x1080,'<i',3);container=f.object('COperativeMission')
        f.put(agent+0x1088,'<Q',int.from_bytes(f.read(container,8),'little'))
        f.put(agent+0x1090,'<Q',agent)
        data=f.object('CDiplomaticPressure@NOperativeMissions');f.put(data+8,'<Q',agent+0x1088);f.put(data+0x10,'<i',2)
        f.put(agent+0x1098,'<Q',data);f.put(agent+0x10A0,'<i',7);f.array(agency+0xF0,[agent])
        r=CountryReader(f,f.world,f.country,1);m=read_intelligence(r)['missions'];r.s.verify()
        self.assertEqual(m['items'][0]['mission_id'],'diplomatic_pressure')
        self.assertEqual(m['items'][0]['target_country'],'ITA')
        plan=compile_watch_reads(r.watch);f.put(agent+0x1080,'<i',0);self.assertTrue(watch_changed(f,plan))
        self.assertEqual(read_intelligence(CountryReader(f,f.world,f.country,1))['missions']['items'][0]['state'],'captured')
        f.put(data+8,'<Q',f.country)
        self.assertEqual(read_intelligence(CountryReader(f,f.world,f.country,1))['missions']['status'],'unavailable')
        f.put(agency+0x78,'<Q',f.country+999)
        with self.assertRaises(LiveReadError):read_intelligence(CountryReader(f,f.world,f.country,1))

    def test_all_eight_mission_types_and_reused_country_tags(self):
        from hoi4companion.foreign_interactions_native import read_intelligence,read_relations,MISSION_CLASSES
        from hoi4companion.foreign_interactions import MISSIONS
        f,dip,agency=self.fixture();agent=f.object('COperativeLeader');f.put(agent+8,'<II',0x1269,1)
        f.put(agent+0x1080,'<i',3);container=f.object('COperativeMission')
        f.put(agent+0x1088,'<Q',int.from_bytes(f.read(container,8),'little'));f.put(agent+0x1090,'<Q',agent)
        f.array(agency+0xF0,[agent])
        for kind in range(1,9):
            data=f.object(MISSION_CLASSES[kind]+'@NOperativeMissions');f.put(data+8,'<Q',agent+0x1088);f.put(data+0x10,'<i',2)
            f.put(agent+0x1098,'<Q',data);f.put(agent+0x10A0,'<i',kind)
            self.assertEqual(read_intelligence(CountryReader(f,f.world,f.country,1))['missions']['items'][0]['mission_id'],MISSIONS[kind])
        tags=int.from_bytes(f.read(f.world+0x358,8),'little');f.string(tags+96,'ITA');f.put(f.world+0x364,'<i',4)
        f.array(f.world+0x340,[0,0,1,1],4)
        self.assertEqual(read_relations(CountryReader(f,f.world,f.country,1),dip)['countries'],['ITA','ROM'])
        # The same textual tag for distinct countries is not a unique target.
        third=f.object('CCountry');f.put(third+8,'<i',3)
        countries=int.from_bytes(f.read(f.world+0x310,8),'little');other=int.from_bytes(f.read(countries+8,8),'little')
        f.array(f.world+0x310,[f.country,other,third]);f.array(f.world+0x340,[0,0,1,2],4)
        self.assertEqual(read_intelligence(CountryReader(f,f.world,f.country,1))['missions']['status'],'unavailable')
