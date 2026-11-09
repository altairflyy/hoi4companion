import copy
import json
import io
from pathlib import Path
import subprocess
import unittest

from hoi4companion.guide import manual_keys
from hoi4companion.guide_importer import preview
from hoi4companion.guide_runtime import evaluate
from hoi4companion.import_sources import acquire_source
from hoi4companion.screen_overlay import targets
from hoi4companion.word_guides import prepare_document
from hoi4companion.guide_authoring import compile_document
from test_word_guides import word


class ConditionalFocusTests(unittest.TestCase):
    def definition(self, **policies):
        line='Reclaim Territory (Bypass) / Hold Exercises (if late with neighbour)'
        source=acquire_source(text=line)
        row={'key':'choice','title':line,'note':line,
             'action':{'kind':'select_focus','entity':{'kind':'focus','id':'Reclaim Territory (Bypass) / Hold Exercises'}},
             'evidence':[{'source_id':source['id'],'basis':'explicit','quote':line}],**policies}
        guide={'id':'conditional','title':'Conditional guide','dependency_policy':'explicit',
               'applies_to':{'country':'TST','tree_id':'tree'},
               'sources':[{'id':source['id'],'type':'manual'}],'tracks':{'focus':[row]}}
        entries=[{'kind':'focus','id':key,'name':name,'scope':'tree','countries':['TST'],'prerequisites':[]}
                 for key,name in [('reclaim','Reclaim Territory'),('exercises','Hold Exercises')]]
        return guide,source,entries

    def state(self, completed=(), active=(), **facts):
        observed=lambda value:{'status':'observed','source':'fixture','value':value}
        values={'country':'TST','focus.tree':'tree','focus.active':list(active),'focus.completed':list(completed),**facts}
        return {'schema_version':1,'usable':True,'facts':{k:observed(v) for k,v in values.items()}}

    def test_import_compiles_bypass_and_conditional_fallback_without_changing_source(self):
        guide,source,entries=self.definition()
        before=copy.deepcopy(guide)
        report=preview(guide,[source],entries)
        self.assertTrue(report['valid'],report['errors'])
        step=report['guide']['tracks']['focus'][0]
        self.assertEqual(step['target'],'exercises')
        self.assertEqual(step['completion'],{'op':'any','args':[
            {'op':'fact','key':'focus.completed','test':'contains','value':'reclaim'},
            {'op':'fact','key':'focus.completed','test':'contains','value':'exercises'}]})
        self.assertEqual(step['action']['params']['conditional_focus']['condition_text'],'late with neighbour')
        self.assertEqual(manual_keys(report['guide']),set())
        self.assertEqual(guide,before)
        again=preview(report['guide'],[source],entries)
        self.assertTrue(again['valid'],again['errors'])
        self.assertEqual(again['guide'],report['guide'])

    def test_runtime_keeps_undefined_condition_unknown_and_accepts_only_qualified_evidence(self):
        guide,source,entries=self.definition()
        report=preview(guide,[source],entries)
        self.assertTrue(report['valid'],report['errors'])
        guide=report['guide'];step=guide['tracks']['focus'][0];guard=step['when']['key']
        states=[self.state(),self.state(completed=['reclaim']),self.state(completed=['exercises']),
                self.state(active=['exercises']),self.state(**{guard:False}),self.state(**{guard:True})]
        stale=copy.deepcopy(states[1]);stale['usable']=False;states.append(stale)
        wrong=copy.deepcopy(states[1]);wrong['facts']['country']['value']='OTHER';states.append(wrong)
        missing=copy.deepcopy(states[0]);del missing['facts']['focus.completed'];states.append(missing)
        results=[evaluate(guide,s) for s in states]
        self.assertEqual([r['steps'][0]['status'] for r in results],
                         ['unverifiable','completed','completed','active','waiting','ready','unverifiable','not_applicable','unverifiable'])
        self.assertEqual(results[0]['steps'][0]['reason'],'condition_unknown')
        for i in (0,4,6,7,8):
            self.assertEqual(targets(guide,results[i],entries,{**states[i],'sample':{'country':'TST'}}),[])
        self.assertEqual([p['identity'] for p in targets(guide,results[5],entries,{**states[5],'sample':{'country':'TST'}})],['exercises'])
        module=(Path(__file__).parent.parent/'src/hoi4companion/web/guide-runtime.js').as_posix()
        script='const r=require(process.argv[1]);let s="";process.stdin.on("data",x=>s+=x);process.stdin.on("end",()=>{const p=JSON.parse(s);process.stdout.write(JSON.stringify(p.states.map(s=>r.evaluate(p.guide,s))));});'
        result=subprocess.run(['node','-e',script,module],input=json.dumps({'guide':guide,'states':states}),text=True,encoding='utf8',capture_output=True,check=True)
        self.assertEqual(json.loads(result.stdout),results)

    def test_existing_policies_and_dependencies_are_preserved(self):
        condition={'op':'fact','key':'explicit.condition','test':'eq','value':True}
        completion={'op':'fact','key':'explicit.result','test':'eq','value':True}
        guide,source,entries=self.definition(when=condition,completion=completion,depends_on_started=['before'])
        guide['tracks']['focus'].insert(0,{'key':'before','action':{'kind':'select_focus','entity':{'kind':'focus','id':'reclaim'}},
                                         'evidence':guide['tracks']['focus'][0]['evidence']})
        report=preview(guide,[source],entries)
        self.assertTrue(report['valid'],report['errors'])
        step=report['guide']['tracks']['focus'][1]
        self.assertEqual((step['when'],step['completion'],step['depends_on_started']),(condition,completion,['before']))
        pending=evaluate(report['guide'],self.state(**{'explicit.condition':True,'explicit.result':False}))
        self.assertEqual(pending['steps'][1]['status'],'waiting')
        self.assertEqual(pending['steps'][1]['blocked_by'],['before'])

    def test_word_source_keeps_conditional_text_and_both_identifiers(self):
        guide,source,entries=self.definition()
        line=guide['tracks']['focus'][0]['title']
        prepared=prepare_document(io.BytesIO(word([[],[],[line]])))
        compiled=compile_document(prepared['document'])
        compiled['guide']['applies_to']=guide['applies_to']
        report=preview(compiled['guide'],compiled['sources'],entries)
        self.assertTrue(report['valid'],report['errors'])
        step=report['guide']['tracks']['focus'][0]
        self.assertEqual(step['action']['params']['conditional_focus']['bypass_id'],'reclaim')
        self.assertEqual(step['action']['params']['conditional_focus']['fallback_id'],'exercises')
        self.assertEqual(step['evidence'][0]['quote'],line)
        self.assertTrue(any('no measurable criterion' in item for item in report['warnings']))

    def test_bare_alternatives_and_inconsistent_titles_do_not_invent_conditions(self):
        guide,source,entries=self.definition()
        row=guide['tracks']['focus'][0]
        row['title']=row['note']='Different Focus (Bypass) / Hold Exercises (if arbitrary)'
        self.assertFalse(preview(guide,[source],entries)['valid'])
        row['title']=row['note']=row['action']['entity']['id']
        self.assertFalse(preview(guide,[source],entries)['valid'])

    def test_unresolved_or_wrong_scope_branch_is_not_silently_discarded(self):
        for change in ('missing','scope','country'):
            guide,source,entries=self.definition()
            if change=='missing':entries.pop()
            elif change=='scope':entries[1]['scope']='other'
            else:entries[1]['countries']=['OTHER']
            report=preview(guide,[source],entries)
            self.assertFalse(report['valid'])
            self.assertNotIn('conditional_focus',report['guide']['tracks']['focus'][0]['action'].get('params',{}))
