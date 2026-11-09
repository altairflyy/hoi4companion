"""Verify the native ZIP using isolated data and its actual Windows binaries."""
import argparse,hashlib,http.client,json,os,socket,subprocess,tempfile,time,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]

def sha(path):
    with Path(path).open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()

def smoke(archive,output):
    archive=Path(archive).resolve(strict=True);result={'schema_version':1,'archive':str(archive),'sha256':sha(archive),'game_started':False,'live_verified':False,'hud_visual_alignment_verified':False}
    with tempfile.TemporaryDirectory(prefix='hoi4-native-smoke-') as temporary:
        folder=Path(temporary)
        with zipfile.ZipFile(archive) as bundle:
            for name in bundle.namelist():
                path=Path(name)
                if path.is_absolute() or '..' in path.parts or ':' in name:raise RuntimeError('Unsafe native ZIP member')
            bundle.extractall(folder)
        app=folder/'HOI4Companion';manifest=json.loads((app/'manifest.json').read_text(encoding='utf8'))
        for name,expected in manifest['files'].items():
            if sha(app/name)!=expected:raise RuntimeError('Package file hash mismatch: '+name)
        result['manifest_files_verified']=len(manifest['files']);result['resident_python']=manifest['resident_python']
        if result['resident_python'] is not False:raise RuntimeError('Native bundle still declares a resident interpreter')
        data=folder/'profile';data.mkdir();(data/'settings.json').write_text(json.dumps({'game_dir':str(data/'unconfigured'),'docs_dir':str(data/'documents'),'ui_language':'english'}),encoding='utf8')
        env={k:v for k,v in os.environ.items() if not k.startswith('HOI4') and not k.startswith('PYTHON')}
        env['PATH']=str(Path(os.environ['SystemRoot'])/'System32');env['HOI4COMPANION_DATA_DIR']=str(data)
        executable=app/'HOI4 Companion.exe';hud=app/'resources/_native/overlay/hoi4-overlay.exe'
        checked=subprocess.run([str(executable),'--self-check'],cwd=app,env=env,capture_output=True,text=True,encoding='utf8',timeout=20,creationflags=subprocess.CREATE_NO_WINDOW)
        if checked.returncode:raise RuntimeError(checked.stderr)
        config=json.loads(checked.stdout)['configuration'];assert Path(config['path'])==data/'guide-progress.sqlite3'
        assert Path(config['ffmpeg']).is_relative_to(app);result['startup_self_check']=True
        script=data/'export-source.txt';script.write_bytes('name="café"'.encode('cp1252'))
        exported=data/'export.json'
        export=subprocess.run([str(app/'resources/_native/hoi4-backend.exe'),'--export-script',str(script),'--script-encoding','windows-1252','--json-duplicates','group','--output',str(exported)],cwd=app,env=env,capture_output=True,timeout=20,creationflags=subprocess.CREATE_NO_WINDOW)
        assert export.returncode==0,export.stderr
        assert json.loads(exported.read_text(encoding='utf8'))['name']=='café'
        result['native_jomini_export_verified']=True
        with socket.socket() as port_socket:port_socket.bind(('127.0.0.1',0));port=port_socket.getsockname()[1]
        command=[str(executable),'--port',str(port),'--no-browser','--no-hud']
        def launch(*extra):
            run=subprocess.run([*command,*extra],cwd=app,env=env,capture_output=True,text=True,encoding='utf8',timeout=45,creationflags=subprocess.CREATE_NO_WINDOW)
            if run.returncode:raise RuntimeError(run.stderr)
            return json.loads(run.stdout)
        def api(path,body=None):
            connection=http.client.HTTPConnection('127.0.0.1',port,timeout=30)
            try:
                connection.request('POST' if body is not None else 'GET',path,json.dumps(body) if body is not None else None,{'Content-Type':'application/json'})
                reply=connection.getresponse();raw=reply.read()
                if reply.status!=200:raise RuntimeError(f'{path}: {reply.status}: '+raw[:1000].decode('utf8',errors='replace'))
                return json.loads(raw) if 'application/json' in reply.getheader('Content-Type','') else raw
            finally:connection.close()
        running=False
        try:
            launch();running=True
            assert api('/api/backend')['resident_python'] is False
            assert api('/api/guides')['guides']==[];result['clean_install_has_no_personal_guides']=True
            for page in ('/','/walkthrough','/importer','/technologies','/analysis','/associations'):
                assert b'<html' in api(page).lower(),page
            result['web_routes_served']=6
            assert 'hoi4-guide-authoring' in api('/api/import/prompt')['prompt']
            source=api('/api/import/source',{'text':'Prepare the front.'})
            guide={'id':'distribution-proof','title':'Distribution proof','sources':[{'id':source['id'],'type':'manual'}],
                   'tracks':{'operations':[{'key':'note','tracking':'reminder','action':{'kind':'manual','params':{'category':'army','instructions':['Prepare the front.']}},
                                           'evidence':[{'source_id':source['id'],'basis':'explicit','quote':source['text']}]}]}}
            preview=api('/api/import/preview',{'guide':guide,'sources':[source]});assert preview['valid'],preview.get('errors')
            assert api('/api/import/install',{'guide':guide,'sources':[source],'reviewed':True})['id']==guide['id']
            api('/api/guide/active',{'guide_id':guide['id']});assert api('/api/guide/projection?live=0')['guide_id']==guide['id']
            result['guide_preview_install_selection_verified']=True
            diagnostic=subprocess.run([str(hud),'--native-protocol-check','--port',str(port)],cwd=app,env=env,capture_output=True,text=True,encoding='utf8',timeout=20,creationflags=subprocess.CREATE_NO_WINDOW)
            if diagnostic.returncode:raise RuntimeError(diagnostic.stderr)
            native=json.loads(diagnostic.stdout);assert native['resident_python'] is False and native['binary_frame_verified'] and native['frame_cache_verified'];assert native['snapshot']['guide_id']==guide['id']
            result['native_hud_protocol_and_binary_frames_verified']=True
            analysis=api('/api/script-analysis')
            assert analysis['demo'] is True and analysis['status']=='partial'
            assert analysis['rules']['adapter']=='cwtools-subset-v2' and analysis['runtime_evaluated'] is False
            assert analysis['coverage']['documents']>0 and isinstance(analysis['diagnostics'],list)
            result['script_analysis_endpoint_verified']=True
            launch('--stop');running=False;time.sleep(.25)
            launch();running=True;assert api('/api/guide/active')['guide_id']==guide['id'];result['installed_guide_survives_restart']=True
            stopped=launch('--stop');running=False;result['owned_stop_verified']=stopped['stopping'] is True
        finally:
            if running:launch('--stop')
        result['external_youtube_download_verified']=False
        result['provider_api_called']=False
    Path(output).write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
    print(json.dumps(result,ensure_ascii=False,indent=2));return result

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('archive');parser.add_argument('--output',default=str(ROOT/'.local/native-windows-smoke-proof.json'))
    args=parser.parse_args();smoke(args.archive,args.output)
