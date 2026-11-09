import copy
import tempfile
import unittest
from pathlib import Path

from hoi4companion.focus import load_continuous_focuses
from hoi4companion.guide_importer import catalog_entities, preview
from hoi4companion.guide import enrich_guide
from hoi4companion.guide_runtime import evaluate
from hoi4companion.import_sources import acquire_source


class ContinuousFocusTests(unittest.TestCase):
    def test_nothing_alternative_is_optional_and_never_blocks_the_next_national_focus(self):
        src=acquire_source(text='Suppress Subjects/Nothing (Until Imperial Federation available)\nImperial Federation')
        guide={'id':'optional','title':'Optional','sources':[{'id':src['id'],'type':'manual'}],
               'tracks':{'focus':[{'key':str(i),'action':{'kind':'select_focus','entity':{'kind':'focus','id':query}},
                 'evidence':[{'source_id':src['id'],'basis':'explicit','quote':query}]} for i,query in enumerate(['Suppress Subjects/Nothing','Imperial Federation'])]}}
        guide['tracks']['focus'][0]['note']=src['text'].splitlines()[0]
        entries=[{'kind':'focus','id':'continuous_restrict_freedom','name':'Suppress Subjects','continuous':True,'scope_independent':True,'prerequisites':[]},
                 {'kind':'focus','id':'federation','name':'Imperial Federation','prerequisites':[]}]
        report=preview(guide,[src],entries)
        self.assertTrue(report['valid'],report['errors'])
        step=report['guide']['tracks']['focus'][0]
        self.assertTrue(step['action']['params']['focus_optional'])
        fact=lambda value:{'status':'observed','source':'fixture','value':value}
        state={'schema_version':1,'usable':True,'facts':{'country':fact('FRA'),'focus.active':fact([]),'focus.continuous':fact([]),'focus.completed':fact([])}}
        runtime=evaluate(report['guide'],state)
        self.assertEqual(runtime['parallel']['focus']['steps'],['1'])
        self.assertEqual(runtime['parallel']['focus']['optional_steps'],['0'])
        self.assertFalse(runtime['steps'][0]['completion']['value'])
        from hoi4companion.screen_overlay import targets
        proposals=targets(report['guide'],runtime,entries,{**state,'sample':{'country':'FRA'}})
        optional=next(p for p in proposals if p['identity']=='continuous_restrict_freedom')
        self.assertEqual(optional['tone'],'choice')
        self.assertEqual({p['identity'] for p in proposals},{'continuous_restrict_freedom','federation'})
        missing=copy.deepcopy(state);del missing['facts']['focus.continuous']
        self.assertEqual(evaluate(report['guide'],missing)['parallel']['focus']['steps'],['1'])
        import subprocess,json
        module=(Path(__file__).parent.parent/'src/hoi4companion/web/guide-runtime.js').as_posix()
        script='const r=require(process.argv[1]);let s="";process.stdin.on("data",x=>s+=x);process.stdin.on("end",()=>{const p=JSON.parse(s);process.stdout.write(JSON.stringify(p.states.map(s=>r.evaluate(p.guide,s))));});'
        result=subprocess.run(['node','-e',script,module],input=json.dumps({'guide':report['guide'],'states':[state,missing]}),text=True,encoding='utf8',capture_output=True,check=True)
        self.assertEqual(json.loads(result.stdout),[evaluate(report['guide'],x) for x in [state,missing]])
        done=copy.deepcopy(state);done['facts']['focus.completed']=fact(['federation'])
        self.assertEqual(evaluate(report['guide'],done)['parallel']['focus']['optional_steps'],[])
        report['guide']['tracks']['focus'][1]['depends_on']=['0']
        self.assertEqual(evaluate(report['guide'],state)['parallel']['focus']['steps'],[])
    def test_existing_imports_are_qualified_in_memory_without_rewriting_the_guide(self):
        from hoi4companion.api import Store
        store=Store.__new__(Store);store.continuous_focuses={'continuous_restrict_freedom':object()}
        guide={'tracks':{'focus':[{'key':'s','action':{'kind':'select_focus','entity':{'kind':'focus','id':'continuous_restrict_freedom'}}}]}}
        before=copy.deepcopy(guide)
        adapted=store._runtime_definition(guide)
        self.assertEqual(adapted['tracks']['focus'][0]['action']['params']['focus_mode'],'continuous')
        self.assertEqual(guide,before)
        self.assertIs(store._runtime_definition(adapted),adapted)
    def test_generic_palette_resolves_with_any_national_tree_without_inserting_layout_nodes(self):
        with tempfile.TemporaryDirectory() as root:
            path=Path(root)/'palette.txt'
            path.write_text('''continuous_focus_palette={id=generic_focus default=yes
                focus={id=continuous_restrict_freedom icon=GFX_generic
                       available={is_subject=no} modifier={autonomy_gain=-0.5}}}''',encoding='utf8')
            focuses,warnings=load_continuous_focuses([path],{'continuous_restrict_freedom':'Suppress Subjects'})
        self.assertFalse(warnings)
        catalog=catalog_entities({}, {}, {}, {'continuous_restrict_freedom':'Suppress Subjects'},continuous_focuses=focuses)
        for country,tree in [('POL','polish'),('GER','german')]:
            source=acquire_source(text='Suppress Subjects')
            guide={'id':'continuous-guide','title':'Generic','applies_to':{'country':country,'tree_id':tree},
                   'sources':[{'id':source['id'],'type':'manual'}],'tracks':{'focus':[{'key':'suppress',
                       'action':{'kind':'select_focus','entity':{'kind':'focus','id':'Suppress Subjects'}},
                       'evidence':[{'source_id':source['id'],'basis':'explicit','quote':'Suppress Subjects'}]}]}}
            result=preview(guide,[source],catalog)
            self.assertTrue(result['valid'],result['errors'])
            step=result['guide']['tracks']['focus'][0]
            self.assertEqual(step['target'],'continuous_restrict_freedom')
            self.assertEqual(step['action']['params']['focus_mode'],'continuous')
            enriched=enrich_guide(result['guide'],{}, {},{}, {},continuous_focuses=focuses)
            self.assertTrue(enriched['tracks']['focus'][0]['catalog_match'])
            self.assertNotIn('estimated_days',enriched['tracks']['focus'][0])

    def test_selection_uses_qualified_continuous_fact_without_completing_the_focus(self):
        guide={'id':'continuous','dependency_policy':'explicit','tracks':{'focus':[
            {'key':'suppress','action':{'kind':'select_focus','entity':{'kind':'focus','id':'continuous_restrict_freedom'},
             'params':{'focus_mode':'continuous'}}},
            {'key':'after_start','action':{'kind':'select_focus','entity':{'kind':'focus','id':'other'}},'depends_on_started':['suppress']},
            {'key':'after_end','action':{'kind':'select_focus','entity':{'kind':'focus','id':'last'}},'depends_on':['suppress']}]}}
        def fact(v):return {'status':'observed','source':'fixture','value':v}
        state={'schema_version':1,'usable':True,'facts':{'focus.active':fact([]),'focus.completed':fact([]),
               'focus.continuous':fact(['continuous_restrict_freedom'])}}
        result=evaluate(guide,state)
        self.assertEqual([r['status'] for r in result['steps']],['active','ready','waiting'])
        self.assertFalse(result['steps'][0]['completion']['value'])
        mistaken_completion=copy.deepcopy(state)
        mistaken_completion['facts']['focus.completed']=fact(['continuous_restrict_freedom'])
        self.assertFalse(evaluate(guide,mistaken_completion)['steps'][0]['completion']['value'])
        unknown=copy.deepcopy(state);del unknown['facts']['focus.continuous']
        self.assertIsNone(evaluate(guide,unknown)['steps'][0]['selection']['value'])
        wrong_fact=copy.deepcopy(unknown);wrong_fact['facts']['focus.active']=fact(['continuous_restrict_freedom'])
        self.assertIsNone(evaluate(guide,wrong_fact)['steps'][0]['selection']['value'])
        guide['tracks']['focus'][0]['action']['params']['focus_mode']='invalid'
        with self.assertRaises(ValueError):evaluate(guide,state)
