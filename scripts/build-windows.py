"""Build a redistributable Windows x64 ZIP from an explicit resource allowlist."""
import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import sys
import tomllib
from urllib.request import urlopen
import zipfile

ROOT=Path(__file__).resolve().parents[1]
NODE_VERSION='24.15.0'


def sha(path):
    with Path(path).open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()


def download_node():
    name=f'node-v{NODE_VERSION}-win-x64';filename=name+'.zip'
    base=f'https://nodejs.org/dist/v{NODE_VERSION}/'
    with urlopen(base+'SHASUMS256.txt',timeout=60) as response:sums=response.read().decode()
    expected=next(line.split()[0] for line in sums.splitlines() if line.split()[-1]==filename)
    cache=ROOT/'.local/windows-build-downloads';cache.mkdir(parents=True,exist_ok=True)
    archive=cache/filename
    if not archive.is_file() or sha(archive)!=expected:
        temporary=archive.with_suffix('.download')
        with urlopen(base+filename,timeout=120) as response,temporary.open('wb') as target:
            shutil.copyfileobj(response,target)
        if sha(temporary)!=expected:raise RuntimeError('Checksum Node non corrispondente')
        temporary.replace(archive)
    destination=cache/name;destination.mkdir(exist_ok=True)
    with zipfile.ZipFile(archive) as bundle:
        for leaf in ('node.exe','LICENSE','README.md'):
            (destination/leaf).write_bytes(bundle.read(name+'/'+leaf))
    (destination/'provenance.json').write_text(json.dumps({'version':NODE_VERSION,
        'url':base+filename,'sha256':expected},indent=2),encoding='utf8')
    return destination


def notices(destination):
    folder=destination/'THIRD-PARTY';folder.mkdir(exist_ok=True)
    records=[]
    for dist in sorted(importlib.metadata.distributions(),key=lambda d:d.metadata['Name'].lower()):
        name=dist.metadata['Name'];entry={'name':name,'version':dist.version,
            'license':dist.metadata.get('License-Expression') or dist.metadata.get('License',''),
            'home_page':dist.metadata.get('Home-page',''),'files':[]}
        for file in dist.files or []:
            if '.dist-info/' in str(file).replace('\\','/') and any(word in str(file).lower() for word in ('license','copying','notice')):
                source=Path(dist.locate_file(file))
                if source.is_file():
                    target=folder/name/Path(file).name;target.parent.mkdir(exist_ok=True)
                    shutil.copyfile(source,target);entry['files'].append(target.relative_to(destination).as_posix())
        records.append(entry)
    python_license=Path(sys.base_prefix)/'LICENSE.txt'
    if python_license.is_file():shutil.copyfile(python_license,folder/'PYTHON-LICENSE.txt')
    import imageio_ffmpeg
    ffmpeg=imageio_ffmpeg.get_ffmpeg_exe()
    flags=getattr(subprocess,'CREATE_NO_WINDOW',0)
    for option,filename in (('-L','FFMPEG-LICENSE.txt'),('-version','FFMPEG-BUILD.txt')):
        result=subprocess.run([ffmpeg,option],capture_output=True,text=True,check=True,creationflags=flags)
        (folder/filename).write_text(result.stdout+result.stderr,encoding='utf8')
    (folder/'packages.json').write_text(json.dumps(records,indent=2),encoding='utf8')
    (folder/'README.txt').write_text('''Componenti distribuiti separatamente; il codice del Companion resta MIT.
Le licenze di parser/overlay Rust sono in _internal/hoi4companion/licenses.
CWTools e il runtime .NET includono avvisi e provenienza nella relativa cartella.
FFmpeg 7.1 essentials (imageio-ffmpeg 0.6.0) e GPLv3; build e configurazione
sono in FFMPEG-BUILD.txt, licenza in FFMPEG-LICENSE.txt.
Origine binari/build/source: https://www.gyan.dev/ffmpeg/builds/
Sorgenti FFmpeg 7.1: https://ffmpeg.org/releases/ffmpeg-7.1.tar.xz
Wrapper Python FFmpeg: https://github.com/imageio/imageio-ffmpeg
Node.js: licenza e provenienza in _internal/hoi4companion/_native/node.
PyInstaller: GPL con eccezione per la distribuzione del bootloader.
Le licenze delle librerie Python sono incluse nelle sottocartelle.
''',encoding='utf8')


def build(skip_native=False):
    if sys.platform!='win32' or sys.maxsize<=2**32:raise RuntimeError('Build richiesta su Windows x64')
    native=ROOT/'src/hoi4companion/_native' if skip_native else ROOT/'build/windows-native'
    native.mkdir(parents=True,exist_ok=True)
    if not skip_native:
        subprocess.run(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(ROOT/'scripts/build-parser.ps1'),'-NoInstall'],cwd=ROOT,check=True)
        shutil.copyfile(ROOT/'.local/rust-target/release/hoi4companion_parser.dll',
                        native/'hoi4companion_parser.dll')
        for name,folder in (('build-cwtools.py','cwtools'),('build-overlay.py','overlay')):
            runpy.run_path(str(ROOT/'scripts'/name))['build'](native/folder)
    node=download_node()
    env=os.environ.copy();env['HOI4_BUILD_NODE']=str(node);env['HOI4_BUILD_NATIVE']=str(native)
    subprocess.run([sys.executable,'-m','PyInstaller','--noconfirm','--clean','--distpath',str(ROOT/'dist'),
        '--workpath',str(ROOT/'build/windows'),str(ROOT/'packaging/windows.spec')],cwd=ROOT,env=env,check=True)
    destination=ROOT/'dist/HOI4Companion'
    for file in ('README.md','LICENSE'):shutil.copyfile(ROOT/file,destination/file)
    shutil.copytree(ROOT/'docs',destination/'docs',dirs_exist_ok=True)
    (destination/'LEGGIMI.txt').write_text('''HOI4 COMPANION - WINDOWS x64

1. Estrai tutta la cartella ZIP in una cartella a scelta.
2. Apri HOI4 Companion.exe. Non spostare i due eseguibili fuori dalla cartella.
3. Si apre il browser su http://127.0.0.1:8766/walkthrough e il controllo HUD.
4. Configura il percorso di HOI4 se non viene trovato automaticamente.
5. Importa la tua guida da Importa guida. L'esempio Polonia e facoltativo.

Non occorrono Python, Rust, .NET, Node o FFmpeg installati separatamente.
Dati personali e log: %LOCALAPPDATA%\\HOI4Companion
Non sono incluse le impostazioni o le guide importate dello sviluppatore.
Il programma NON gioca al tuo posto. Le evidenziazioni sono trasparenti ai clic.
La lettura live richiede una build di HOI4 supportata. Consulta README.md.
Questa build non e firmata digitalmente: Windows puo chiedere una verifica.
Per usare solo il browser: HOI4 Companion.exe --no-hud
''',encoding='utf8')
    notices(destination)
    version=tomllib.loads((ROOT/'pyproject.toml').read_text(encoding='utf8'))['project']['version']
    revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    manifest={'version':version,'platform':'windows-x64','revision':revision,
        'working_tree_dirty':bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)),
        'data_directory':'%LOCALAPPDATA%/HOI4Companion','files':{}}
    for path in sorted(destination.rglob('*')):
        if path.is_file():manifest['files'][path.relative_to(destination).as_posix()]=sha(path)
    forbidden=('.local/','settings.json','guide-progress.sqlite3','.env','/guide-videos/','/sprite-cache/')
    if any(any(part in name for part in forbidden) for name in manifest['files']):raise RuntimeError('Dati locali rilevati nel pacchetto')
    (destination/'manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf8')
    archive=ROOT/f'dist/HOI4Companion-{version}-windows-x64.zip'
    with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=6) as output:
        for path in sorted(destination.rglob('*')):
            if path.is_file():output.write(path,'HOI4Companion/'+path.relative_to(destination).as_posix())
    checksum=archive.with_suffix('.zip.sha256');checksum.write_text(sha(archive)+'  '+archive.name+'\n',encoding='ascii')
    print(json.dumps({'zip':str(archive),'bytes':archive.stat().st_size,'sha256':sha(archive)},indent=2))
    return archive


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--skip-native',action='store_true',help='Riusa i componenti nativi gia compilati, solo per sviluppo')
    build(parser.parse_args().skip_native)
