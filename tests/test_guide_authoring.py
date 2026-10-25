import copy
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from types import SimpleNamespace
from unittest.mock import patch

from hoi4companion.guide_authoring import compile_document, action_reference
from hoi4companion.guide_importer import Importer
from hoi4companion.guide_runtime import evaluate


def document(*steps):
    quote='Build two factories. Choose answer. Research tools.'
    return {'format':'hoi4-guide-authoring','authoring_schema_version':1,
            'sources':[{'id':'s','type':'manual','title':'Fixture','text':quote,'segments':[]}],
            'guide':{'id':'generic-authoring','title':'Fixture','applies_to':{},
                     'steps':[{'key':f's{i}','title':s['type'],'evidence':[{'source_id':'s','basis':'explicit','quote':quote}],**s}
                              for i,s in enumerate(steps)]}}


CATALOG=[{'kind':'building','id':'arms_factory','name':'Military Factory'},
         {'kind':'state','id':'55','name':'Example State'},
         {'kind':'equipment','id':'rifles','name':'Rifles'},
         {'kind':'technology','id':'tools','name':'Tools'},
         {'kind':'focus','id':'a','name':'First Focus','scope':'tree','prerequisites':[]},
         {'kind':'project','id':'project','name':'Example Project'},
         {'kind':'law','id':'law','name':'Example Law','group':'economy'}]


class AuthoringTests(unittest.TestCase):
    def test_simple_document_runs_through_real_importer_and_leaves_input_untouched(self):
        src=document({'type':'build','building':'Military Factory','state':'Example State','quantity':{'mode':'add','value':2}},
                     {'type':'research','target':'Tools'})
        before=copy.deepcopy(src)
        loaded=Importer(lambda:CATALOG).document(src)
        self.assertEqual(src,before)
        self.assertEqual(loaded['import_schema_version'],1)
        self.assertNotIn('steps',loaded['guide'])
        self.assertEqual(loaded['guide']['tracks']['research'][0]['key'],'s1')
        result=Importer(lambda:CATALOG).preview(loaded['guide'],loaded['sources'])
        self.assertTrue(result['valid'],result['errors'])
        self.assertEqual(result['operations'][0]['reason'],'baseline_from_live')
        self.assertEqual(result['guide']['tracks']['operations'][0]['action']['params']['state_id'],55)
        self.assertEqual(Importer(lambda:CATALOG).document(loaded),loaded)

    def test_build_add_total_and_baseline_produce_distinct_goals(self):
        for mode,target in [('add',7),('total',2)]:
            loaded=Importer(lambda:CATALOG).document(document({'type':'build','building':{'id':'arms_factory'},
                'state':{'id':55},'quantity':{'mode':mode,'value':2},'baseline':5}))
            result=Importer(lambda:CATALOG).preview(loaded['guide'],loaded['sources'])
            self.assertTrue(result['valid'],result['errors'])
            s=result['guide']['tracks']['operations'][0]
            self.assertEqual(s['completion']['value'],target)
            self.assertEqual(s['action']['params']['quantity_mode'],'additional' if mode=='add' else 'target_total')

    def test_missing_fields_invalid_quantities_and_conflicting_details_fail_precisely(self):
        base={'type':'build','building':'Military Factory','state':'Example State','quantity':{'mode':'add','value':2}}
        for field in ['building','state','quantity']:
            s=copy.deepcopy(base);s.pop(field)
            with self.subTest(field=field),self.assertRaisesRegex(ValueError,'s0.*'+field):compile_document(document(s))
        for value in [True,-1,0,1.5,float('nan')]:
            s=copy.deepcopy(base);s['quantity']['value']=value
            with self.subTest(value=value),self.assertRaises(ValueError):compile_document(document(s))
        for details in [{'quantity':99},{'category':'politics'},{'production_selector':{}},{'law_id':'law'}]:
            with self.subTest(details=details),self.assertRaises(ValueError):compile_document(document({**base,'details':details}))

    def test_production_layout_preserves_requested_vs_assigned_and_regenerates_binding(self):
        for basis in ['requested','assigned']:
            d=document({'type':'production','lines':[{'equipment':{'id':'rifles'},'factories':{'basis':basis,'value':10}}]})
            loaded=Importer(lambda:CATALOG).document(d)
            report=Importer(lambda:CATALOG).preview(loaded['guide'],loaded['sources'])
            self.assertTrue(report['valid'],report['errors'])
            state={'usable':True,'sample':{'country':'TST','country_overview':{'country':'TST','production':{'status':'observed','lines':[
                {'id':'l','equipment':'rifles','name':'Rifles','category':'military','requested_factories':10,'assigned_factories':3,'active_factories':3,'damaged_factories':0}]}}}}
            result=evaluate(report['guide'],state)['steps'][0]
            self.assertEqual(result['status'],'completed' if basis=='requested' else 'active')

    def test_all_action_types_have_explicit_v1_mappings(self):
        steps=[{'type':'focus','target':{'id':'a','scope':'tree'}},
               {'type':'research','target':'Tools','upgrade_from':'tools'},
               {'type':'special_project','target':'Example Project'},
               {'type':'law','target':{'id':'law'}}, {'type':'idea','target':'Spirit'},
               {'type':'appoint','person':'Example Person','role':'army_chief'},
               {'type':'event','event':{'id':'event.1'},'option':{'id':'event.1.a'}},
               {'type':'decision','target':'Example Decision'},
               {'type':'create_agency'}, {'type':'agency_upgrade','target':'Air Department'},
               {'type':'intelligence_operation','operation':'collaboration','country':'TST'},
               {'type':'mio','organization':'Example Designer'},
               {'type':'template','template':'Infantry','details':{'battalions_add':['infantry']}},
               {'type':'design','name':'Example Tank','details':{'chassis':'medium','modules':['gun']}},
               {'type':'army','task':'Train divisions','details':{'division_count':4}},
               {'type':'diplomacy','task':'Guarantee','country':'TST'},
               {'type':'instruction','category':'checkpoints','instructions':['Wait for the declared condition.']},
               {'type':'reminder','category':'army','instructions':['Keep reserves.']},
               {'type':'free','lane':'research'}]
        g=compile_document(document(*steps))['guide'];rows={s['key']:s for r in g['tracks'].values() for s in r}
        self.assertEqual(len(rows),len(steps))
        self.assertEqual(rows['s0']['action']['entity'],{'kind':'focus','id':'a','scope':'tree'})
        self.assertEqual(rows['s3']['action']['params']['law_id'],'law')
        self.assertEqual(rows['s6']['action']['params']['event_option_id'],'event.1.a')
        self.assertEqual(rows['s8']['action']['params']['operation_id'],'create_intelligence_agency')
        self.assertNotIn('project_name',rows['s9']['action']['params'])
        self.assertEqual(rows['s12']['action']['params']['template_name'],'Infantry')
        self.assertEqual(rows['s17']['tracking'],'reminder')
        self.assertNotIn('completion',rows['s17'])
        self.assertEqual(rows['s18']['action']['kind'],'free')

    def test_advanced_v1_keeps_all_policies_and_never_silently_catches_unknown_types(self):
        original={'key':'advanced','title':'Explicit policy','action':{'kind':'manual','params':{'category':'army'}},
                  'completion':{'op':'check','key':'done'},'evidence':[{'source_id':'s','basis':'explicit','quote':'Build two factories. Choose answer. Research tools.'}]}
        d=document({'type':'advanced_v1','step':original,'key':original['key'],'title':original['title']})
        g=compile_document(d)['guide']
        for field in original:self.assertEqual(g['tracks']['operations'][0][field],original[field])
        with self.assertRaises(ValueError):compile_document(document({'type':'made_up'}))
        bad=document({'type':'research','target':'Tools','details':{'interpretation':'next_upgrade'}})
        self.assertEqual(compile_document(bad)['guide']['tracks']['research'][0]['action']['params']['interpretation'],'next_upgrade')

    def test_dependencies_sources_reminders_and_versions_are_validated(self):
        d=document({'type':'research','target':'Tools','depends_on':['s1']},{'type':'research','target':'Tools','depends_on':['s0']})
        with self.assertRaisesRegex(ValueError,'ciclic'):compile_document(d)
        d=document({'type':'reminder','instructions':['Advice']},{'type':'research','target':'Tools','depends_on':['s0']})
        with self.assertRaisesRegex(ValueError,'promemoria'):compile_document(d)
        for version in [True,2,'1']:
            d=document({'type':'free'});d['authoring_schema_version']=version
            with self.assertRaises(ValueError):compile_document(d)
        d=document({'type':'research','target':'Tools'});d['guide']['steps'][0]['evidence'][0]['source_id']='missing'
        with self.assertRaises(ValueError):compile_document(d)

    def test_unresolved_names_remain_visible_and_ambiguous_catalog_does_not_pick_first(self):
        loaded=Importer(lambda:CATALOG).document(document({'type':'research','target':'Tools'}))
        report=Importer(lambda:CATALOG+[{'kind':'technology','id':'other','name':'Tools'}]).preview(loaded['guide'],loaded['sources'])
        self.assertFalse(report['valid']);self.assertEqual(report['resolutions'][0]['status'],'ambiguous')

    def test_file_installs_v1_only_without_authoring_or_progress_data(self):
        with TemporaryDirectory() as folder:
            importer=Importer(lambda:CATALOG,folder)
            loaded=importer.document(document({'type':'research','target':'Tools'}))
            result=importer.install(loaded['guide'],loaded['sources'],reviewed=True)
            saved=json.loads(Path(result['path']).read_text(encoding='utf8'))
            self.assertEqual(saved['schema_version'],1);self.assertNotIn('steps',saved)
            self.assertNotIn('authoring_schema_version',saved)

    def test_action_reference_exposes_the_same_supported_types(self):
        names={row['type'] for row in action_reference()}
        self.assertTrue({'build','production','law','appoint','event','decision','template','design','diplomacy','instruction','advanced_v1'}<=names)

    def test_explicit_v1_equivalent_has_identical_runtime_and_javascript_results(self):
        import subprocess
        d=document({'type':'build','building':'Military Factory','state':'Example State','quantity':{'mode':'add','value':2},'baseline':5},
                   {'type':'research','target':'Tools','depends_on':[]},
                   {'type':'law','target':{'id':'law'},'depends_on':['s0']})
        evidence=copy.deepcopy(d['guide']['steps'][0]['evidence'])
        # Independent v1 reference: not generated by the authoring compiler.
        old={'format':'hoi4-guide-import','import_schema_version':1,'sources':copy.deepcopy(d['sources']),
             'guide':{'id':'generic-authoring','title':'Fixture','applies_to':{},'sources':[{'id':'s','type':'manual'}],
                      'tracks':{'focus':[],'operations':[
                          {'key':'s0','title':'build','evidence':evidence,'action':{'kind':'manual','params':{'category':'construction','building_name':'Military Factory','state_name':'Example State','quantity':2,'quantity_mode':'additional','baseline_level':5}}},
                          {'key':'s2','title':'law','evidence':evidence,'depends_on':['s0'],'action':{'kind':'manual','params':{'category':'politics','law_id':'law'}}}],
                          'research':[{'key':'s1','title':'research','evidence':evidence,'depends_on':[],'action':{'kind':'research','entity':{'kind':'technology','id':'Tools'}}}]}}}
        importer=Importer(lambda:CATALOG)
        reports=[]
        for doc in [old,d]:
            loaded=importer.document(json.loads(json.dumps(doc)));report=importer.preview(loaded['guide'],loaded['sources'])
            self.assertTrue(report['valid'],report['errors']);reports.append(report)
        self.assertEqual(reports[0]['guide'],reports[1]['guide'])
        self.assertEqual(reports[0]['operations'],reports[1]['operations'])
        observed=lambda v:{'status':'observed','value':v,'source':'fixture'}
        states=[{'schema_version':1,'usable':True,'facts':{
            'country':observed('TST'),'country.states.items.55.military_factories':observed(count),
            'construction.queued':observed(queue),'research.completed':observed([]),'research.active':observed([]),
            'research.slots':observed([{'technology_id':None}]),'politics.ideas':observed(ideas)}}
            for count,queue,ideas in [(5,[],[]),(5,['state:55:arms_factory'],[]),(7,[],['law'])]]
        states += [{'schema_version':1,'usable':False,'facts':{}},{'schema_version':1,'usable':True,'facts':{}}]
        payload={'guides':[r['guide'] for r in reports],'states':states}
        script="const x=JSON.parse(require('fs').readFileSync(0,'utf8'));process.stdout.write(JSON.stringify(x.guides.map(g=>x.states.map(s=>require('./src/hoi4companion/web/guide-runtime.js').evaluate(g,s)))));"
        js=json.loads(subprocess.run(['node','-e',script],input=json.dumps(payload),encoding='utf8',capture_output=True,check=True).stdout)
        expected=[[evaluate(r['guide'],s) for s in states] for r in reports]
        self.assertEqual(expected[0],expected[1]);self.assertEqual(js,expected)

    def test_event_parent_mismatch_is_never_bound_to_the_wrong_choice(self):
        catalog=CATALOG+[{'kind':'event','id':'event.1','name':'One'}, {'kind':'event','id':'event.2','name':'Two'},
                         {'kind':'event_option','id':'event.2.a','name':'Accept','event_id':'event.2'}]
        importer=Importer(lambda:catalog)
        loaded=importer.document(document({'type':'event','event':{'id':'event.1'},'option':{'id':'event.2.a'}}))
        report=importer.preview(loaded['guide'],loaded['sources'])
        self.assertEqual(report['operations'][0]['reason'],'event_identity_unresolved')
        self.assertNotIn('started_when',report['guide']['tracks']['operations'][0])

    def test_nested_generated_fields_and_invalid_scalar_fields_do_not_bypass_validation(self):
        for details in [{'production_targets':[{'production_selector':{}}]},{'nested':{'political_binding':{}}}]:
            with self.assertRaises(ValueError):compile_document(document({'type':'instruction','category':'production','instructions':['Order'],'details':details}))
        for field,value in [('timing',{}),('source_seconds',True),('source_seconds',-1),('game_date','1937.2.29')]:
            with self.subTest(field=field),self.assertRaises(ValueError):compile_document(document({'type':'research','target':'Tools',field:value}))
        for value in [None,[],{},True]:
            with self.assertRaises(ValueError):compile_document(document({'type':value}))

    def test_optional_provider_compiles_authoring_and_reuses_the_supplied_sources(self):
        import io
        from hoi4companion.import_sources import acquire_source
        src=acquire_source(text='Research Tools.')
        d=document({'type':'research','target':'Tools'})
        d['guide']['steps'][0]['evidence']=[{'source_id':src['id'],'basis':'explicit','quote':'Research Tools.'}]
        d['sources']=[{'id':'invented','type':'manual','text':'Changed source'}]
        class Response(io.BytesIO):
            def __enter__(self):return self
            def __exit__(self,*args):self.close()
        response={'status':'completed','output':[{'type':'message','content':[{'type':'output_text','text':json.dumps(d)}]}]}
        with patch.dict('os.environ',{'OPENAI_API_KEY':'test-key','HOI4_IMPORT_MODEL':'test-model'}),patch('hoi4companion.guide_importer.build_opener') as opener:
            opener.return_value.open.return_value=Response(json.dumps(response).encode())
            result=Importer(lambda:CATALOG).extract([src])
            self.assertTrue(result['valid'],result['errors'])
            self.assertEqual(result['guide']['sources'][0]['id'],src['id'])
            self.assertIn('hoi4-guide-authoring',json.loads(opener.return_value.open.call_args.args[0].data)['instructions'])

    def test_http_document_and_preview_accept_both_formats_and_reject_missing_fields(self):
        from http.server import ThreadingHTTPServer
        import threading
        from urllib.request import Request,urlopen
        from urllib.error import HTTPError
        from hoi4companion.api import make_handler
        importer=Importer(lambda:CATALOG)
        with ThreadingHTTPServer(('127.0.0.1',0),make_handler(SimpleNamespace(importer=lambda:importer))) as server:
            worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start()
            def post(path,body):
                request=Request(f'http://127.0.0.1:{server.server_port}'+path,data=json.dumps(body).encode(),headers={'Content-Type':'application/json'})
                with urlopen(request,timeout=10) as r:return json.load(r)
            try:
                d=document({'type':'research','target':'Tools'})
                loaded=post('/api/import/document',{'document':d})
                self.assertTrue(post('/api/import/preview',{'guide':loaded['guide'],'sources':loaded['sources']})['valid'])
                self.assertEqual(post('/api/import/document',{'document':loaded}),loaded)
                d['guide']['steps']=[{'key':'bad','type':'build','building':'Military Factory','quantity':{'mode':'add','value':2}}]
                with self.assertRaises(HTTPError) as error:post('/api/import/document',{'document':d})
                self.assertEqual(error.exception.code,400)
                self.assertIn('bad: manca state',json.load(error.exception)['error'])
            finally:server.shutdown();worker.join(timeout=3)


if __name__=='__main__':unittest.main()
