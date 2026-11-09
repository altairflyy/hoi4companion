"""The Rust HTTP process serves coherent packets without a Python service."""
import http.client
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest

ROOT=Path(__file__).resolve().parents[1]


class BackendHttpTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.folder=tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.folder.cleanup)
        folder=Path(cls.folder.name)
        from imageio_ffmpeg import get_ffmpeg_exe
        config=folder/'config.json';ready=folder/'ready.json'
        config.write_text(json.dumps({'path':str(folder/'progress.sqlite3'),
            'selection_path':str(folder/'active-guide.json'),'web_root':str(ROOT/'src/hoi4companion/web'),
            'reference_library': str(folder/'reference-library'),'ffmpeg':get_ffmpeg_exe(),
            'catalog':{'settings':{'game_dir':'','country':'AUS'},'settings_path':str(folder/'settings.json'),'demo_file':str(ROOT/'src/hoi4companion/examples/demo_focus.txt'),'demo_labels':str(ROOT/'src/hoi4companion/examples/demo_labels.json')},
            'guides':[{'id':x,'tracks':{'operations':[{'key':'note','action':{'kind':'manual'}}]}} for x in ('one','two')]}),encoding='utf8')
        executable=os.environ.get('HOI4_BACKEND_EXE',str(ROOT/'src/hoi4companion/_native/hoi4-backend.exe'))
        cls.process=subprocess.Popen([executable,'--config',str(config),'--port','0','--ready-file',str(ready)],
            stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,text=True)
        def stop():
            if cls.process.poll() is None:cls.process.terminate()
            cls.process.communicate(timeout=10)
        cls.addClassCleanup(stop)
        deadline=time.monotonic()+10
        while not ready.exists():
            if cls.process.poll() is not None:raise AssertionError(cls.process.stderr.read())
            if time.monotonic()>deadline:raise AssertionError('Rust HTTP startup timed out')
            time.sleep(.02)
        cls.port=json.loads(ready.read_text())['port']

    def request(self,path,method='GET',body=None,headers=None):
        connection=http.client.HTTPConnection('127.0.0.1',self.port,timeout=5)
        self.addCleanup(connection.close)
        h={'Content-Type':'application/json'}
        h.update(headers or {})
        connection.request(method,path,json.dumps(body) if body is not None else None,h)
        reply=connection.getresponse();data=reply.read()
        return reply.status,dict(reply.getheaders()),data

    def test_native_projection_runtime_and_web_assets(self):
        status,_,data=self.request('/api/guide/projection?id=one&live=0')
        self.assertEqual(status,200)
        packet=json.loads(data);self.assertEqual(packet['engine'],'rust-guide-core')
        self.assertEqual(packet['guide_id'],packet['progress']['guide_id'])
        status,headers,data=self.request('/walkthrough')
        self.assertEqual(status,200);self.assertIn(b'<html',data.lower())
        self.assertIn("default-src 'self'",headers['Content-Security-Policy'])
        self.assertEqual(self.request('/%2e%2e/settings.json')[0],404)

    def test_native_listener_accepts_request_body_after_a_short_network_delay(self):
        import socket
        payload=json.dumps({'schema_version':1,'kind':'snapshot','token':99}).encode()
        with socket.create_connection(('127.0.0.1',self.port),timeout=5) as connection:
            connection.sendall(f'POST /api/overlay HTTP/1.1\r\nHost: 127.0.0.1:{self.port}\r\nContent-Type: application/json\r\nContent-Length: {len(payload)}\r\nConnection: close\r\n\r\n'.encode())
            time.sleep(.05)
            connection.sendall(payload)
            response=http.client.HTTPResponse(connection);response.begin()
            self.assertEqual(response.status,200);self.assertEqual(json.loads(response.read())['token'],99)

    def test_native_hud_diagnostics_reject_wrong_context_and_expired_status(self):
        active=json.loads(self.request('/api/guide/active')[2])
        path=Path(self.folder.name)/'overlay-status.json'
        self.addCleanup(lambda:path.unlink(missing_ok=True))
        value={'guide_id':active['guide_id'],'guide_server_id':active['server_id'],
               'status_message':'Open Production','status_code':'panel_closed','explanations':[
                   {'code':'panel_closed','message':'Open Production','step_keys':['note']}]}
        path.write_text(json.dumps(value),encoding='utf8');time.sleep(.27)
        packet=json.loads(self.request('/api/state')[2])
        self.assertEqual(packet['overlay']['status'],'observed')
        self.assertEqual(packet['overlay']['items'][0]['step_keys'],['note'])
        value['guide_server_id']='old-server';path.write_text(json.dumps(value),encoding='utf8');time.sleep(.27)
        self.assertEqual(json.loads(self.request('/api/state')[2])['overlay']['code'],'context_mismatch')
        os.utime(path,(time.time()-10,time.time()-10));time.sleep(.27)
        self.assertEqual(json.loads(self.request('/api/state')[2])['overlay']['code'],'stale')

    def test_native_hud_tracks_app_selection_and_never_draws_from_disconnected_state(self):
        original=json.loads(self.request('/api/guide/active')[2])['guide_id']
        self.addCleanup(lambda:self.request('/api/guide/active','POST',{'guide_id':original}))
        def overlay(kind,**extra):
            status,_,data=self.request('/api/overlay','POST',{'schema_version':1,'kind':kind,'token':17,**extra})
            self.assertEqual(status,200);return json.loads(data)
        value=overlay('select',guide_id='two')
        self.assertEqual(value['guide_id'],'two');self.assertEqual(value['token'],17)
        self.assertEqual({g['id'] for g in value['guides']},{g['id'] for g in json.loads(self.request('/api/guides')[2])['guides']})
        value=overlay('observe',view=[99,42,0,0,400,300])
        self.assertFalse(value['usable']);self.assertFalse(value['native_available']);self.assertEqual(value['matches'],[])
        self.assertEqual(value['guide_id'],json.loads(self.request('/api/guide/active')[2])['guide_id'])
        version=value['version'];self.assertEqual(overlay('snapshot')['version'],version)
        overlay('select',guide_id='one');self.assertNotEqual(overlay('snapshot')['version'],version)
        self.assertEqual(self.request('/api/overlay','POST',{'schema_version':1,'kind':'observe','token':0,'view':[99,42,0,0,0,300]})[0],400)
        self.assertEqual(self.request('/api/overlay','POST',{'schema_version':1,'kind':'select','token':0,'guide_id':'absent'})[0],400)

    def test_catalog_routes_share_one_generation_and_refresh_rejects_parameters(self):
        status,_,data=self.request('/api/config');self.assertEqual(status,200)
        info=json.loads(data);self.assertTrue(info['demo']);self.assertTrue(info['trees'])
        key=info['trees'][0]['id']
        status,_,data=self.request('/api/tree?id='+key+'&source=live');self.assertEqual(status,200)
        tree=json.loads(data);self.assertEqual(tree['id'],key);self.assertFalse(tree['has_snapshot'])
        self.assertEqual(json.loads(self.request('/api/technologies')[2])['folders'],[])
        self.assertIn('error',json.loads(self.request('/api/technology-tree?id=missing')[2]))
        self.assertEqual(self.request('/api/sprite/not-registered.png')[0],404)
        self.assertEqual(self.request('/api/refresh','POST',{'extra':True})[0],400)
        self.assertEqual(self.request('/api/refresh','POST',{})[0],200)

    def test_active_guide_and_guarded_progress_endpoints(self):
        rows=json.loads(self.request('/api/guides')[2])['guides']
        self.assertTrue(all(row['compatibility']=={'blocked':False,'status':'unknown','requirements':[],'unknown':True} for row in rows))
        self.assertEqual(self.request('/api/guide/active','POST',{'guide_id':'two'})[0],200)
        self.assertEqual(json.loads(self.request('/api/guide/active')[2])['guide_id'],'two')
        progress=json.loads(self.request('/api/guide/progress?id=two')[2])
        update={'run_id':progress['run_id'],'checks':{'note':True},'expected_revision':progress['run_revision'],'server_id':progress['server_id']}
        self.assertEqual(self.request('/api/guide/progress?id=two','POST',update)[0],200)
        self.assertEqual(self.request('/api/guide/progress?id=two','POST',update)[0],400)
        self.assertEqual(json.loads(self.request('/api/guide/progress?id=two')[2])['checks'],{'note':True})

    def test_host_origin_and_content_framing_are_checked_before_mutation(self):
        self.assertEqual(self.request('/api/guide/active','POST',{'guide_id':'one'},{'Origin':'https://external.example'})[0],403)
        self.assertEqual(self.request('/api/guides',headers={'Host':'external.example'})[0],403)
        self.assertEqual(self.request('/api/guide/active','POST',{'guide_id':'one'},{'Content-Type':'text/plain'})[0],415)
        self.assertEqual(self.request('/api/guide/projection?live=wrong')[0],400)

    def test_sse_publishes_state_and_closes_subscription(self):
        connection=http.client.HTTPConnection('127.0.0.1',self.port,timeout=5);self.addCleanup(connection.close)
        connection.request('GET','/api/state/events');reply=connection.getresponse()
        self.assertEqual(reply.status,200);self.assertIn('text/event-stream',reply.getheader('Content-Type'))
        lines=[]
        for _ in range(10):
            line=reply.readline();lines.append(line)
            if line.startswith(b'data: '):break
        self.assertIn(b'event: state\n',lines)
        value=json.loads(next(line[6:] for line in lines if line.startswith(b'data: ')))
        self.assertFalse(value['usable']);self.assertIn('guide_server_id',value)

    def test_ui_language_persists_without_reloading_catalog_or_rewriting_guide(self):
        guide=json.loads(self.request('/api/guide?id=one')[2])
        self.assertEqual(self.request('/api/config','POST',{'ui_language':'italian'})[0],200)
        self.assertEqual(json.loads((Path(self.folder.name)/'settings.json').read_text())['ui_language'],'italian')
        html=self.request('/importer')[2].decode('utf8');self.assertIn('lang="it"',html);self.assertIn('Importa guida',html)
        script=self.request('/i18n.js')[2].decode('utf8');self.assertTrue(script.startswith('globalThis.HOI_TRANSLATIONS='))
        self.assertEqual(guide,json.loads(self.request('/api/guide?id=one')[2]))
        self.assertEqual(self.request('/api/config','POST',{'ui_language':'unsupported'})[0],400)
        self.assertEqual(self.request('/api/config','POST',{'ui_language':'english'})[0],200)

    def test_import_catalog_preserves_local_resolver_names_and_compacts_ai_attachment(self):
        status,_,body=self.request('/api/import/catalog');self.assertEqual(status,200)
        catalog=json.loads(body);self.assertTrue(catalog['entities']);self.assertTrue(catalog['demo'])
        self.assertIn('localized_name',catalog['entities'][0])
        status,_,body=self.request('/api/import/catalog?export=ai');self.assertEqual(status,200)
        ai=json.loads(body);self.assertEqual(ai['localization_policy'],'local_lookup')
        self.assertNotIn('localized_name',ai['entities'][0])
        self.assertEqual(self.request('/api/import/catalog?export=invalid')[0],400)

    def test_legacy_checklist_updates_share_the_same_guarded_run(self):
        self.assertEqual(self.request('/api/guide/checks?id=one','POST',{'key':'note','checked':True})[0],200)
        self.assertTrue(json.loads(self.request('/api/guide/checks?id=one')[2])['checks']['note'])
        self.assertEqual(self.request('/api/guide/checks?id=one','POST',{'key':'not-declared','checked':True})[0],400)
        self.assertTrue(json.loads(self.request('/api/guide/progress?id=one')[2])['checks']['note'])
        self.assertEqual(self.request('/api/guide/checks?id=one','POST',{'reset':True})[0],200)
        self.assertEqual(json.loads(self.request('/api/guide/checks?id=one')[2])['checks'],{})

    def test_import_prompt_and_document_compile_without_a_python_service(self):
        status,_,body=self.request('/api/import/prompt');self.assertEqual(status,200)
        self.assertIn('hoi4-guide-authoring',json.loads(body)['prompt'])
        raw={'format':'hoi4-guide-authoring','authoring_schema_version':1,'sources':[{'id':'s','type':'manual','text':'Research Tools.'}],
             'guide':{'id':'external','title':'External','steps':[{'type':'research','target':'Tools','evidence':[{'source_id':'s','basis':'explicit','quote':'Research Tools.'}]}]}}
        status,_,body=self.request('/api/import/document','POST',{'document':raw});self.assertEqual(status,200)
        document=json.loads(body);self.assertEqual(document['import_schema_version'],1)
        self.assertTrue(document['sources'][0]['id'].startswith('src-'))
        self.assertEqual(document['guide']['tracks']['research'][0]['evidence'][0]['source_id'],document['sources'][0]['id'])

    def test_reference_library_upload_inventory_and_image_without_python_service(self):
        from test_reference_designs import archive
        connection=http.client.HTTPConnection('127.0.0.1',self.port,timeout=10)
        self.addCleanup(connection.close)
        connection.request('POST','/api/import/templates',archive(),{'Content-Type':'application/octet-stream'})
        response=connection.getresponse();body=response.read();self.assertEqual(response.status,200,body)
        uploaded=json.loads(body);self.assertEqual(len(uploaded['designs']),4)
        status,_,body=self.request('/api/reference-library');self.assertEqual(status,200)
        inventory=json.loads(body);self.assertEqual(inventory['designs'],uploaded['designs'])
        self.assertIn('game_presets',inventory)
        status,headers,data=self.request('/api/reference-library/image?id='+uploaded['images'][0]['id'])
        self.assertEqual(status,200);self.assertTrue(data.startswith(b'\x89PNG'));self.assertEqual(headers['Content-Type'],'image/png')
        self.assertEqual(self.request('/api/reference-library/image?id=invalid')[0],404)

    def test_import_packet_preview_and_status_use_native_catalog_without_live_reads(self):
        from hoi4companion.import_sources import acquire_source
        status,_,data=self.request('/api/import/status');self.assertEqual(status,200)
        self.assertEqual(json.loads(data)['modes'],['external_json','openai_api'])
        entries=json.loads(self.request('/api/import/catalog')[2])['entities']
        focus=next(row for row in entries if row['kind']=='focus')
        source=acquire_source(text='Take '+focus['name']+'.')
        guide={'id':'native-preview','title':'Fixture','sources':[{'id':source['id'],'type':'manual'}],
            'tracks':{'focus':[{'key':'first','action':{'kind':'select_focus','entity':{'kind':'focus','id':focus['id'],'scope':focus['scope']}},
                'evidence':[{'source_id':source['id'],'basis':'explicit','quote':source['text']}]}]}}
        before=self.request('/api/guide/active')[2]
        status,_,data=self.request('/api/import/preview','POST',{'guide':guide,'sources':[source]})
        self.assertEqual(status,200,data);report=json.loads(data);self.assertTrue(report['valid'],report['errors'])
        self.assertEqual(report['availability']['status'],'ready')
        self.assertEqual(report['guide']['tracks']['focus'][0]['target'],focus['id'])
        status,_,data=self.request('/api/import/packet','POST',{'sources':[source],'tree_id':focus['scope']})
        self.assertEqual(status,200,data);packet=json.loads(data)
        self.assertEqual(packet['sources'],[source]);self.assertEqual(packet['context'],{'tree_id':focus['scope']})
        self.assertNotIn('localized_name',packet['catalog'][0]);self.assertIn('hoi4-guide-authoring',packet['instructions'])
        self.assertEqual(self.request('/api/guide/active')[2],before)
        self.assertEqual(self.request('/api/import/packet','POST',{'sources':[source],'tree_id':'absent'})[0],400)

    def test_install_requires_review_and_preserves_existing_guides_atomically(self):
        from hoi4companion.import_sources import acquire_source
        source=acquire_source(text='Prepare the front.')
        guide={'id':'native-installed','title':'Installed fixture','sources':[{'id':source['id'],'type':'manual'}],
               'tracks':{'operations':[{'key':'note','tracking':'reminder','action':{'kind':'manual','params':{'instructions':[source['text']]}},
                   'evidence':[{'source_id':source['id'],'basis':'explicit','quote':source['text']}]}]}}
        packet={'guide':guide,'sources':[source]}
        path=Path(self.folder.name)/'guides/native-installed.json'
        self.assertEqual(self.request('/api/import/install','POST',packet)[0],400);self.assertFalse(path.exists())
        before=self.request('/api/guide/active')[2]
        status,_,data=self.request('/api/import/install','POST',{**packet,'reviewed':True})
        self.assertEqual(status,200,data);result=json.loads(data);self.assertEqual(result['id'],guide['id'])
        persisted=path.read_bytes();self.assertEqual(json.loads(persisted)['title'],guide['title'])
        self.assertEqual(json.loads(self.request('/api/guide?id=native-installed')[2])['id'],guide['id'])
        ids={row['id'] for row in json.loads(self.request('/api/guides')[2])['guides']}
        self.assertTrue({'one','two',guide['id']}<=ids)
        self.assertEqual(self.request('/api/guide/active')[2],before)
        self.assertEqual(self.request('/api/import/install','POST',{**packet,'reviewed':True})[0],400)
        self.assertEqual(path.read_bytes(),persisted)
        self.assertFalse(list(path.parent.glob('*.tmp')))

    def test_general_settings_reload_atomically_and_reject_invalid_fields(self):
        before=json.loads(self.request('/api/config')[2])
        self.assertEqual(self.request('/api/config','POST',{'country':'HUN','language':'italian','mods':'off'})[0],200)
        updated=json.loads(self.request('/api/config')[2]);self.assertEqual(updated['settings']['country'],'HUN')
        self.assertEqual(updated['settings']['mods'],'off');self.assertTrue(updated['demo'])
        for invalid in ({'country':'HU'},{'mods':'unknown'},{'language':'invalid'},{'game_dir':None},{'extra':'value'}):
            self.assertEqual(self.request('/api/config','POST',invalid)[0],400)
            self.assertEqual(json.loads(self.request('/api/config')[2]),updated)
        self.assertEqual(self.request('/api/config','POST',{key:before['settings'][key] for key in ('country','language','mods')})[0],200)

    def test_word_upload_is_streamed_and_prepared_by_native_reader(self):
        from test_word_guides import word
        payload=word([['Create Agency'],['Basic Machine Tools'],['Focus']])
        def upload(path,headers=None):
            connection=http.client.HTTPConnection('127.0.0.1',self.port,timeout=5);self.addCleanup(connection.close)
            connection.request('POST',path,payload,{'Content-Type':'application/octet-stream',**(headers or {})})
            reply=connection.getresponse();return reply.status,json.loads(reply.read())
        status,inventory=upload('/api/import/word');self.assertEqual(status,200);self.assertTrue(inventory['single_document'])
        status,packet=upload('/api/import/word?prepare=1');self.assertEqual(status,200)
        self.assertEqual(packet['document']['guide']['steps'][0]['type'],'create_agency')
        self.assertEqual(upload('/api/import/word',{'Origin':'https://external.example'})[0],403)

    def test_association_editor_preserves_source_run_and_rejects_stale_updates(self):
        from hoi4companion.import_sources import acquire_source
        entries=json.loads(self.request('/api/import/catalog')[2])['entities']
        first=next(r for r in entries if r['kind']=='focus')
        second=next(r for r in entries if r['kind']=='focus' and r['scope']==first['scope'] and r['id']!=first['id'])
        source=acquire_source(text='Take '+first['name']+'.')
        entity=lambda r:{'kind':'focus','id':r['id'],'scope':r['scope']}
        guide={'id':'native-association-guide','title':'Association fixture','sources':[{'id':source['id'],'type':'manual'}],
            'applies_to':{'tree_id':first['scope']},'tracks':{'focus':[{'key':'first','action':{'kind':'select_focus','entity':entity(first)},
            'evidence':[{'source_id':source['id'],'basis':'explicit','quote':source['text']}]}]}}
        status,_,data=self.request('/api/import/install','POST',{'guide':guide,'sources':[source],'reviewed':True})
        self.assertEqual(status,200,data)
        path=Path(json.loads(data)['path']);before=path.read_bytes()
        progress=self.request('/api/guide/progress?id='+guide['id'])[2]
        update={'guide_id':guide['id'],'key':'first','expected':entity(first),'entity':entity(second)}
        status,_,data=self.request('/api/guide/associations','POST',update)
        self.assertEqual(status,200,data)
        current=json.loads(data);self.assertEqual(current['guide']['tracks']['focus'][0]['target'],second['id'])
        self.assertEqual(len(current['changes']),1)
        self.assertEqual(self.request('/api/guide/associations','POST',update)[0],400)
        self.assertEqual(self.request('/api/guide/associations','POST',{**update,'extra':True})[0],400)
        status,_,data=self.request('/api/guide/associations?id='+guide['id']);self.assertEqual(status,200)
        self.assertEqual(json.loads(data)['changes'],current['changes'])
        status,_,data=self.request('/api/guide/associations','POST',{**update,'expected':entity(second),'entity':None})
        self.assertEqual(status,200,data);self.assertEqual(json.loads(data)['changes'],[])
        self.assertEqual(path.read_bytes(),before)
        self.assertEqual(self.request('/api/guide/progress?id='+guide['id'])[2],progress)


    def test_pinned_script_rule_endpoint_matches_existing_descriptions(self):
        from hoi4companion.script_analysis import RuleBook
        for kind,name in [('trigger','has_completed_focus'),('effect','set_capital'),('trigger','unknown_mod_predicate')]:
            status,_,data=self.request('/api/script-rules?kind='+kind+'&name='+name)
            self.assertEqual(status,200,data);self.assertEqual(json.loads(data),RuleBook.load().describe(kind,name))
        self.assertEqual(self.request('/api/script-rules?kind=invalid&name=a')[0],400)
        self.assertEqual(self.request('/api/script-rules?kind=effect&name=')[0],400)

    def test_demo_analysis_is_native_cached_and_matches_partial_reference(self):
        from hoi4companion.script_analysis import analyze_documents
        from hoi4companion.jomini_parser import parse_file
        source=ROOT/'src/hoi4companion/examples/demo_focus.txt'
        labels=json.loads((ROOT/'src/hoi4companion/examples/demo_labels.json').read_text(encoding='utf8'))
        expected=analyze_documents([('common/national_focus/demo.txt',parse_file(source))],labels,labels)
        expected['demo']=True
        status,_,data=self.request('/api/script-analysis?refresh=1');self.assertEqual(status,200,data)
        actual=json.loads(data);self.assertEqual(actual,expected)
        self.assertEqual(self.request('/api/script-analysis')[2],data)
        self.assertFalse(actual['runtime_evaluated']);self.assertEqual(actual['rules']['adapter'],'cwtools-subset-v2')

    def test_native_import_endpoints_reject_unknown_fields_and_non_objects(self):
        for operation in ('document','packet','preview','install'):
            self.assertEqual(self.request('/api/import/'+operation,'POST',{'unexpected':True})[0],400)
            self.assertEqual(self.request('/api/import/'+operation,'POST',[])[0],400)

    def test_native_media_endpoint_extracts_registered_frame_and_supports_clip_ranges(self):
        from imageio_ffmpeg import get_ffmpeg_exe
        from hoi4companion.import_sources import acquire_source
        url='https://www.youtube.com/watch?v=abcdefghijk'
        root=Path(self.folder.name);source=root/'source.mp4';frames=root/'guide-frames';frames.mkdir(exist_ok=True)
        subprocess.run([get_ffmpeg_exe(),'-v','error','-nostdin','-f','lavfi','-i','color=c=green:s=160x90:r=15','-t','2','-c:v','libx264','-y',str(source)],capture_output=True,check=True,timeout=25)
        (frames/'sources.json').write_text(json.dumps({url:{'path':str(source),'duration':2}}),encoding='utf8')
        src=acquire_source(url=url,text='Prepare the front.')
        guide={'id':'native-media','title':'Media fixture','sources':[{'id':src['id'],'type':'youtube','url':url}],
            'tracks':{'operations':[{'key':'note','tracking':'reminder','action':{'kind':'manual','params':{'source_seconds':.5,'instructions':[src['text']]}},'evidence':[{'source_id':src['id'],'basis':'explicit','quote':src['text']}]}]}}
        status,_,data=self.request('/api/import/install','POST',{'guide':guide,'sources':[src],'reviewed':True});self.assertEqual(status,200,data)
        suffix='?id=native-media&key=note'
        status,_,data=self.request('/api/guide/media'+suffix);self.assertEqual(status,200,data)
        references=json.loads(data);self.assertTrue(references['references'][0]['source_available'])
        self.assertTrue(references['references'][0]['time_available']);self.assertEqual(references['references'][0]['clip_end'],2)
        status,headers,data=self.request('/api/guide/frame'+suffix);self.assertEqual(status,200,data);self.assertTrue(data.startswith(b'\xff\xd8'));self.assertEqual(headers['Content-Type'],'image/jpeg')
        self.assertEqual(self.request('/api/guide/frame'+suffix+'&ref=missing')[0],404)
        self.assertEqual(self.request('/api/guide/frame'+suffix+'&ref=')[0],404)
        status,headers,clip=self.request('/api/guide/clip'+suffix);self.assertEqual(status,200,clip[:100])
        status,headers,data=self.request('/api/guide/clip'+suffix,headers={'Range':'bytes=0-19'});self.assertEqual(status,206);self.assertEqual(data,clip[:20])
        self.assertEqual(headers['Content-Range'],'bytes 0-19/'+str(len(clip)))
        self.assertEqual(self.request('/api/guide/clip'+suffix,headers={'Range':'bytes=0-1,2-3'})[0],416)
        self.assertEqual(self.request('/api/import/frame?url=https%3A%2F%2Fyoutu.be%2Fabcdefghijk&seconds=.5')[0],200)
        self.assertEqual(self.request('/api/import/frame?url=file:///C:/Windows/test&seconds=0')[0],404)

    def test_native_source_endpoint_preserves_text_and_rejects_local_targets(self):
        from hoi4companion.import_sources import acquire_source
        status,_,data=self.request('/api/import/source','POST',{'text':'Research Tools.'})
        self.assertEqual(status,200,data);self.assertEqual(json.loads(data),acquire_source(text='Research Tools.'))
        for update in ({'url':'http://127.0.0.1/'},{'url':'file:///C:/Windows'},{'text':'x','extra':True}):
            self.assertEqual(self.request('/api/import/source','POST',update)[0],400)

    def test_native_video_job_prepares_registered_source_audio_cache_and_downloads(self):
        from imageio_ffmpeg import get_ffmpeg_exe
        url='https://www.youtube.com/watch?v=lmnopqrstuv'
        root=Path(self.folder.name);source=root/'video-job-source.mp4';frames=root/'guide-frames';frames.mkdir(exist_ok=True)
        subprocess.run([get_ffmpeg_exe(),'-v','error','-nostdin','-f','lavfi','-i','color=c=red:s=160x90:r=15','-f','lavfi','-i','sine=frequency=400:sample_rate=44100','-t','3','-c:v','libx264','-c:a','aac','-y',str(source)],capture_output=True,check=True,timeout=25)
        registry=frames/'sources.json';items=json.loads(registry.read_text(encoding='utf8'))if registry.is_file()else{}
        items[url]={'path':str(source),'duration':3,'title':'Video test','description':'Test source'};registry.write_text(json.dumps(items),encoding='utf8')
        before=source.read_bytes()
        status,_,data=self.request('/api/import/video','POST',{'url':url})
        self.assertEqual(status,200,data);job=json.loads(data);key=job['id'];deadline=time.monotonic()+90
        while job['status']=='running':
            self.assertLess(time.monotonic(),deadline,job)
            time.sleep(.05);status,_,data=self.request('/api/import/video/status?id='+key);self.assertEqual(status,200,data);job=json.loads(data)
        self.assertEqual(job['status'],'completed',job)
        self.assertTrue(job['cache_reused']);self.assertEqual(job['audio_status'],'completed');self.assertTrue(job['parts'])
        status,headers,data=self.request('/api/import/video/file?id='+key+'&name=manifest.json');self.assertEqual(status,200,data)
        manifest=json.loads(data);self.assertEqual((manifest['fps'],manifest['segment_seconds'],manifest['overlap_seconds'],manifest['max_part_bytes']),(15,300,10,150000000))
        self.assertEqual(manifest['audio']['codec'],'aac');self.assertFalse(manifest['audio']['transcoded'])
        self.assertEqual(self.request('/api/import/video/file?id='+key+'&name='+manifest['parts'][0]['name'])[0],200)
        self.assertEqual(self.request('/api/import/video/file?id='+key+'&name=audio.m4a')[0],200)
        self.assertEqual(self.request('/api/import/video/file?id='+key+'&name=../video-job-source.mp4')[0],400)
        status,_,data=self.request('/api/import/video','POST',{'url':url});self.assertEqual(status,200,data);self.assertEqual(json.loads(data)['id'],key)
        self.assertEqual(self.request('/api/import/video','POST',{'url':url,'refresh_source':'yes'})[0],400)
        self.assertEqual(source.read_bytes(),before)

    def test_catalog_reload_refreshes_runtime_presentation_without_changing_run(self):
        before=json.loads(self.request('/api/guide?id=one')[2]);progress=json.loads(self.request('/api/guide/progress?id=one')[2])
        status,_,data=self.request('/api/config','POST',{'country':'ROM'});self.assertEqual(status,200,data)
        current=json.loads(self.request('/api/guide?id=one')[2]);self.assertEqual(current['catalog_country'],'ROM')
        self.assertNotEqual(current['runtime_definition_revision'],before['runtime_definition_revision'])
        after=json.loads(self.request('/api/guide/progress?id=one')[2]);self.assertEqual(after['run_id'],progress['run_id']);self.assertEqual(after['checks'],progress['checks'])
        self.assertEqual(self.request('/api/config','POST',{'country':'AUS'})[0],200)
