"""Exercise the extracted distribution with no Python/.NET/Node/FFmpeg on PATH."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import shutil
import zipfile
from urllib.request import urlopen


def run(folder):
    folder=Path(folder).resolve();runtime=folder/'Companion Runtime.exe'
    manifest=json.loads((folder/'manifest.json').read_text(encoding='utf8'))
    for name,digest in manifest['files'].items():
        with (folder/name).open('rb') as stream:assert hashlib.file_digest(stream,'sha256').hexdigest()==digest,name
    with tempfile.TemporaryDirectory(prefix='HOI4 distribuzione pulita ') as temporary:
        data=Path(temporary)/'dati utente';data.mkdir()
        env={k:v for k,v in os.environ.items() if not k.startswith(('HOI4_','HOI4COMPANION_','PYTHON'))}
        env['HOI4COMPANION_DATA_DIR']=str(data)
        env['PATH']=os.environ['SystemRoot']+'\\System32;'+os.environ['SystemRoot']
        flags=getattr(subprocess,'CREATE_NO_WINDOW',0)
        result=subprocess.run([runtime,'--self-check',str(data/'self-check.json')],env=env,capture_output=True,text=True,timeout=180,creationflags=flags)
        if result.returncode:raise RuntimeError(result.stdout+result.stderr)
        checks=json.loads((data/'self-check.json').read_text(encoding='utf8'))
        assert checks['cwtools_engine_executed']=='cwtools' and checks['hevc_encode_verified']
        assert checks['node_version'] and Path(checks['data_directory'])==data
        with socket.socket() as listener:listener.bind(('127.0.0.1',0));port=listener.getsockname()[1]
        # Force the demo, without touching the current user's installed HOI4.
        (data/'settings.json').write_text(json.dumps({'game_dir':str(data/'no-game'),'docs_dir':str(data/'no-docs'),'mods':'off'}))
        with (data/'server.log').open('w',encoding='utf8') as log:
            server=subprocess.Popen([runtime,'-m','hoi4companion','--port',str(port),'--no-browser'],env=env,stdout=log,stderr=log,creationflags=flags)
        base=f'http://127.0.0.1:{port}'
        def get(path):
            with urlopen(base+path,timeout=15) as response:return json.load(response)
        bridge=None
        try:
            deadline=time.monotonic()+90
            while True:
                try:config=get('/api/config');break
                except OSError:
                    if server.poll() is not None or time.monotonic()>deadline:raise RuntimeError((data/'server.log').read_text())
                    time.sleep(.3)
            assert config['application']=='hoi4companion' and config['demo'] is True
            for path in ('/','/technologies','/walkthrough','/importer','/country','/analysis','/importer.js','/guide-runtime.js'):
                with urlopen(base+path) as response:assert response.status==200
            guides=get('/api/guides')['guides'];assert not any('kaiserin' in g['id'] for g in guides)
            assert get('/api/import/catalog')['entities']
            assert 'hoi4-guide-authoring' in get('/api/import/prompt')['prompt']
            bridge=subprocess.Popen([runtime,'-m','hoi4companion.overlay_bridge','--port',str(port)],
                env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,creationflags=flags)
            requests=b'{"schema_version":1,"kind":"snapshot","token":1}\n{"schema_version":1,"kind":"close","token":2}\n'
            output,errors=bridge.communicate(requests,timeout=30)
            if bridge.returncode:raise RuntimeError(errors.decode(errors='replace'))
            replies=[json.loads(line) for line in output.splitlines()]
            assert [r['token'] for r in replies]==[1,2] and all(r['ok'] for r in replies)
            launch=subprocess.run([folder/'HOI4 Companion.exe','--port',str(port),'--no-browser','--no-hud'],env=env,timeout=20,creationflags=flags)
            assert launch.returncode==0
            ytdlp=subprocess.run([runtime,'-m','yt_dlp','--version'],env=env,capture_output=True,text=True,timeout=30,creationflags=flags)
            assert ytdlp.returncode==0
            checks.update(http_pages_verified=8,clean_profile=True,personal_guides_absent=True,
                          overlay_binary_protocol_verified=True,desktop_launcher_reuses_service=True,
                          ytdlp_worker_version=ytdlp.stdout.strip(),no_external_tools_on_path=True,
                          game_live_validation=False)
            return checks
        finally:
            if bridge and bridge.poll() is None:bridge.kill();bridge.wait(timeout=10)
            server.terminate();server.wait(timeout=15)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('folder',type=Path);parser.add_argument('--output',type=Path)
    args=parser.parse_args()
    if args.folder.suffix.lower()=='.zip':
        with tempfile.TemporaryDirectory(prefix='HOI4 estratto con spazi ') as folder:
            root=Path(folder)
            with zipfile.ZipFile(args.folder) as archive:
                if sum(i.file_size for i in archive.infolist())>2_000_000_000:raise ValueError('Archivio oltre 2 GB estratti')
                for info in archive.infolist():
                    target=(root/info.filename).resolve()
                    if not target.is_relative_to(root.resolve()):raise ValueError('Percorso ZIP non valido')
                    if info.is_dir():target.mkdir(parents=True,exist_ok=True);continue
                    target.parent.mkdir(parents=True,exist_ok=True)
                    with archive.open(info) as source,target.open('wb') as output:shutil.copyfileobj(source,output)
            result=run(root/'HOI4Companion')
            result['zip_extraction_and_relocation_verified']=True
    else:result=run(args.folder)
    text=json.dumps(result,ensure_ascii=False,indent=2)
    if args.output:args.output.write_text(text,encoding='utf8')
    print(text)
