import copy
import io
import json
from pathlib import Path
import subprocess
import unittest

from hoi4companion.guide_authoring import compile_document
from hoi4companion.guide_importer import preview
from hoi4companion.guide_runtime import evaluate
from hoi4companion.screen_overlay import targets
from hoi4companion.word_guides import prepare_document
from test_word_guides import word


class InformalOperationsTests(unittest.TestCase):
    def laws(self):
        return [{'kind':'law','id':key,'name':name,'group':group,'aliases':[]}
                for key,name,group in [('war_economy','War Economy','economic_laws'),
                                       ('limited_conscription','Limited Conscription','mobilization_laws'),
                                       ('free_trade','Free Trade','trade_laws')]]

    def catalog(self):
        return [{'kind':'advisor','id':key,'character_id':key+'_person','name':name,'aliases':[trait],
                 'slot':role,'countries':['TST']}
                for key,name,trait,role in [('infantry','First Person','Infantry\n(Expert)','high_command'),
                                           ('jurist','Second Person','Monarchist Jurist','political_advisor')]]

    def report(self, lines, catalog=None, edit=None):
        packet=prepare_document(io.BytesIO(word([lines,[],[]])))
        document=compile_document(packet['document'])
        document['guide']['applies_to']={'country':'TST'}
        if edit:edit(document['guide'])
        before=copy.deepcopy(document['guide'])
        result=preview(document['guide'],document['sources'],catalog if catalog is not None else self.catalog())
        self.assertEqual(document['guide'],before)
        self.assertTrue(result['valid'],result['errors'])
        return result,document['sources']

    def state(self, **facts):
        base={'country':'TST','politics.advisors':[],'country.intelligence.in_creation':False,
              'country.intelligence.is_created':False,'country.politics.political_power':200}
        return {'schema_version':1,'usable':True,'facts':{key:{'status':'observed','source':'fixture','value':value}
                 for key,value in {**base,**facts}.items()}}

    def test_agency_start_and_completion_are_live_but_setup_is_a_reminder(self):
        result,sources=self.report(['Create Agency & Setup Done'])
        rows=result['guide']['tracks']['operations']
        agency=next(r for r in rows if r['action']['params'].get('operation_id')=='create_intelligence_agency')
        self.assertNotEqual(agency.get('tracking'),'reminder')
        self.assertTrue(any(r.get('tracking')=='reminder' and 'Setup Done' in str(r) for r in rows))
        states=[self.state(),self.state(**{'country.intelligence.in_creation':True}),
                self.state(**{'country.intelligence.is_created':True})]
        self.assertEqual([next(r for r in evaluate(result['guide'],s)['steps'] if r['key']==agency['key'])['status']
                          for s in states],['ready','active','completed'])
        progress={'schema_version':1,'guide_id':result['guide']['id'],'run_id':'fixture','checks':{agency['key']:True}}
        checked=next(r for r in evaluate(result['guide'],states[0],progress)['steps'] if r['key']==agency['key'])
        self.assertFalse(checked['completion']['value'])
        again=preview(result['guide'],sources,self.catalog())
        self.assertEqual(again['guide'],result['guide'])

    def test_nested_laws_keep_categories_costs_source_and_order(self):
        line='Prepare the front\n- War Economy (150PP)\n- Keep training\n- Limited Conscription (127PP)\n- Free Trade (150PP)'
        result,sources=self.report([line],self.laws())
        rows=result['guide']['tracks']['operations']
        laws=[r for r in rows if r['action']['params'].get('law_id')]
        self.assertEqual([r['action']['params']['law_id'] for r in laws],['war_economy','limited_conscription','free_trade'])
        self.assertEqual([r['action']['params']['law_group'] for r in laws],['economic_laws','mobilization_laws','trade_laws'])
        self.assertEqual([r['political_power_cost'] for r in laws],[150,127,150])
        self.assertTrue(all(r['evidence'][0]['quote']==line for r in rows))
        self.assertTrue(any(r.get('tracking')=='reminder' and 'Keep training' in str(r) for r in rows))
        self.assertEqual(preview(result['guide'],sources,self.laws())['guide'],result['guide'])
        state=self.state(**{'politics.ideas':[],'country.politics.political_power':100})
        runtime=evaluate(result['guide'],state)
        self.assertEqual(runtime['action_sequence']['political'],laws[0]['key'])
        self.assertEqual(targets(result['guide'],runtime,self.laws(),{**state,'sample':{'country':'TST'}}),[])
        state=self.state(**{'politics.ideas':['war_economy']})
        runtime=evaluate(result['guide'],state)
        self.assertEqual(runtime['action_sequence']['political'],laws[1]['key'])
        self.assertEqual([p['identity'] for p in targets(result['guide'],runtime,self.laws(),{**state,'sample':{'country':'TST'}})],['limited_conscription'])
        missing=self.state(**{'politics.advisors':['war_economy']})
        self.assertIsNone(next(r for r in evaluate(result['guide'],missing)['steps'] if r['key']==laws[0]['key'])['completion']['value'])

    def test_inline_law_condition_is_not_dropped_and_explicit_predicate_wins(self):
        result,_=self.report(['After the war: War Economy (150PP)'],self.laws())
        row=result['guide']['tracks']['operations'][0]
        self.assertEqual(row['action']['params']['law_id'],'war_economy')
        self.assertEqual(row['action']['params']['timing'],'After the war')
        runtime=evaluate(result['guide'],self.state(**{'politics.ideas':[]}))
        self.assertEqual(runtime['steps'][0]['reason'],'condition_unknown')
        predicate={'op':'fact','key':'qualified.phase','test':'eq','value':True}
        def edit(guide):guide['tracks']['operations'][0]['when']=predicate
        qualified,_=self.report(['After the war: War Economy (150PP)'],self.laws(),edit=edit)
        self.assertEqual(qualified['guide']['tracks']['operations'][0]['when'],predicate)
        self.assertEqual(evaluate(qualified['guide'],self.state(**{'politics.ideas':[],'qualified.phase':True}))['steps'][0]['status'],'ready')

    def test_missing_or_ambiguous_laws_and_compound_methods_remain_reminders(self):
        for entries,line in [(self.laws()[:1],'Prepare\n- Limited Conscription (150PP)'),
                             ([*self.laws(),{**self.laws()[0],'id':'another'}],'Prepare\n- War Economy (150PP)'),
                             (self.laws(),'Prepare\n- War Economy (150PP) if possible and train troops')]:
            result,_=self.report([line],entries)
            self.assertTrue(all(r.get('tracking')=='reminder' for r in result['guide']['tracks']['operations']))

    def test_cost_role_order_and_actual_appointment_membership(self):
        report,_=self.report(['Infantry Expert (100PP)','Monarchist Jurist (150PP)'])
        rows=report['guide']['tracks']['operations']
        self.assertEqual([r['action']['params']['advisor_id'] for r in rows],['infantry','jurist'])
        self.assertEqual([r['action']['params']['role'] for r in rows],['high_command','political_advisor'])
        self.assertEqual([r['political_power_cost'] for r in rows],[100,150])
        states=[self.state(**{'country.politics.political_power':50}),self.state(),
                self.state(**{'politics.advisors':['infantry']}),self.state(**{'politics.advisors':['infantry','jurist']})]
        results=[evaluate(report['guide'],s) for s in states]
        self.assertEqual([r['action_sequence']['political'] for r in results],[rows[0]['key'],rows[0]['key'],rows[1]['key'],None])
        self.assertEqual(results[0]['steps'][0]['status'],'waiting')
        self.assertFalse(results[1]['steps'][0]['completion']['value'])
        unknown=self.state();del unknown['facts']['country.politics.political_power']
        self.assertEqual(targets(report['guide'],evaluate(report['guide'],unknown),self.catalog(),{**unknown,'sample':{'country':'TST'}}),[])
        for i in (1,2):
            proposals=targets(report['guide'],results[i],self.catalog(),{**states[i],'sample':{'country':'TST'}})
            self.assertEqual([p['identity'] for p in proposals],['infantry' if i==1 else 'jurist'])
        module=(Path(__file__).parent.parent/'src/hoi4companion/web/guide-runtime.js').as_posix()
        script='const r=require(process.argv[1]);let s="";process.stdin.on("data",x=>s+=x);process.stdin.on("end",()=>{const p=JSON.parse(s);process.stdout.write(JSON.stringify(p.states.map(s=>r.evaluate(p.guide,s))));});'
        out=subprocess.run(['node','-e',script,module],input=json.dumps({'guide':report['guide'],'states':states}),text=True,capture_output=True,check=True)
        self.assertEqual(json.loads(out.stdout),results)

    def test_multiline_context_is_preserved_and_undefined_timing_is_not_guessed(self):
        line='On Civil War: Force deploy\n- Infantry Expert (100PP)\n- Keep the front stable'
        report,_=self.report([line,'Monarchist Jurist (150PP)'])
        rows=report['guide']['tracks']['operations']
        infantry=next(r for r in rows if r['action']['params'].get('advisor_id')=='infantry')
        self.assertIn('On Civil War',infantry['action']['params']['timing'])
        self.assertEqual(infantry['evidence'][0]['quote'],line)
        self.assertIn('when',infantry)
        self.assertTrue(any(r.get('tracking')=='reminder' and 'Force deploy' in str(r) for r in rows))
        runtime=evaluate(report['guide'],self.state())
        self.assertEqual(next(r for r in runtime['steps'] if r['key']==infantry['key'])['reason'],'condition_unknown')
        self.assertEqual(targets(report['guide'],runtime,self.catalog(),{**self.state(),'sample':{'country':'TST'}}),[])

    def test_wrong_country_or_missing_observation_never_confirms_and_reimport_is_stable(self):
        report,sources=self.report(['Infantry Expert (100PP)'])
        guide=report['guide']
        self.assertEqual(preview(guide,sources,self.catalog())['guide'],guide)
        missing=self.state();del missing['facts']['politics.advisors']
        stale=self.state(**{'politics.advisors':['infantry']});stale['usable']=False
        foreign=self.state(**{'country':'OTHER','politics.advisors':['infantry']})
        self.assertIsNone(evaluate(guide,missing)['steps'][0]['completion']['value'])
        self.assertIsNone(evaluate(guide,stale)['steps'][0]['completion']['value'])
        self.assertEqual(evaluate(guide,foreign)['steps'][0]['status'],'not_applicable')

    def test_unscoped_advisor_and_setup_only_do_not_become_observed_actions(self):
        def edit(guide):guide['applies_to']={}
        report,_=self.report(['Infantry Expert (100PP)','Setup Done'],edit=edit)
        self.assertTrue(all(r.get('tracking')=='reminder' for r in report['guide']['tracks']['operations']))

    def test_existing_word_import_is_adapted_in_memory_and_catalog_changes_invalidate_cache(self):
        from hoi4companion.api import Store
        from hoi4companion.resolution import CatalogIndex
        packet=prepare_document(io.BytesIO(word([['Create Agency & Setup Done','Infantry Expert (100PP)'],[],[]])))
        original=compile_document(packet['document'])['guide']
        original['applies_to']={'country':'TST'}
        before=copy.deepcopy(original)
        store=Store.__new__(Store)
        store.operation_entities=self.catalog()
        index=CatalogIndex(self.catalog())
        store.resolution_index=lambda:index
        adapted=store._runtime_definition(original)
        self.assertEqual(original,before)
        self.assertIsNot(adapted,original)
        self.assertIs(store._runtime_definition(original),adapted)
        agency=next(r for r in adapted['tracks']['operations'] if r['action']['params'].get('operation_id'))
        self.assertIn('started_when',agency)
        self.assertIn('completion',agency)
        self.assertTrue(any(r['action']['params'].get('advisor_id')=='infantry' for r in adapted['tracks']['operations']))
        index=CatalogIndex([])
        revised=store._runtime_definition(original)
        self.assertFalse(any(r['action']['params'].get('advisor_id') for r in revised['tracks']['operations']))
        self.assertEqual(original,before)

    def test_ambiguous_foreign_missing_and_compound_labels_stay_reminders(self):
        duplicate={**self.catalog()[0],'id':'other','character_id':'other_person'}
        for catalog,query in [([*self.catalog(),duplicate],'Infantry Expert (100PP)'),
                              ([{**self.catalog()[0],'countries':['OTHER']}],'Infantry Expert (100PP)'),
                              (self.catalog(),'Unknown (100PP)'),
                              (self.catalog(),'Infantry Expert (100PP) and train troops')]:
            report,_=self.report([query],catalog)
            self.assertTrue(all(r.get('tracking')=='reminder' for r in report['guide']['tracks']['operations']))

    def test_authored_conditions_and_dependencies_are_not_overwritten(self):
        def edit(guide):
            rows=guide['tracks']['operations']
            rows[0]['when']={'op':'fact','key':'qualified.phase','test':'eq','value':True}
            rows[1].update(when={'op':'fact','key':'qualified.phase','test':'eq','value':True})
            rows[1]['depends_on_started']=[rows[0]['key']]
        report,_=self.report(['Create Agency','Infantry Expert (100PP)'],edit=edit)
        rows=report['guide']['tracks']['operations']
        infantry=next(r for r in rows if r['action']['params'].get('advisor_id')=='infantry')
        self.assertEqual(infantry['political_power_cost'],100)
        self.assertEqual(infantry['when'],{'op':'fact','key':'qualified.phase','test':'eq','value':True})
        self.assertEqual(len(infantry['depends_on_started']),1)
        self.assertEqual(next(r for r in rows if r['key']==infantry['depends_on_started'][0])['action']['params']['operation_id'],'create_intelligence_agency')
        waiting=evaluate(report['guide'],self.state(**{'qualified.phase':True}))
        self.assertEqual(next(r for r in waiting['steps'] if r['key']==infantry['key'])['status'],'waiting')
        active=evaluate(report['guide'],self.state(**{'qualified.phase':True,'country.intelligence.in_creation':True}))
        self.assertEqual(next(r for r in active['steps'] if r['key']==infantry['key'])['status'],'ready')
