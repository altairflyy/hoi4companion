"""Build the Rust-only Windows application from an explicit resource allowlist.

Python is a development/build tool here; no Python application runtime, source
guide, user settings, saved campaign or compiler cache enters the distribution.
"""
import argparse,hashlib,json,os,shutil,subprocess,tempfile,tomllib,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
PACKAGE=ROOT/'src/hoi4companion'
RESOURCE_FOLDERS=('web','examples','licenses','rules')
RESOURCE_FILES=('guide-import-prompt.md','guide_language.json','authoring-actions-v1.json','foreign-tasks-v1.json','live-profiles-v1.json','design-profile-v1.json','country-functions-v1.json','translations.json')

def sha(path):
    with Path(path).open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()

def locate_tool(name,explicit):
    if explicit:
        path=Path(explicit).resolve(strict=True)
        if not path.is_file():raise ValueError('Tool path must be an executable file')
        return path
    candidates=[PACKAGE/'_native/tools'/name]
    if name=='ffmpeg.exe':candidates+=sorted((ROOT/'.local/media-tools/imageio_ffmpeg/binaries').glob('ffmpeg*.exe'))
    if name=='yt-dlp.exe':candidates.append(Path.home()/'Downloads/yt-dlp.exe')
    found=shutil.which(name)
    if found:candidates.append(Path(found))
    path=next((p for p in candidates if p.is_file()),None)
    if path is None:raise ValueError('Standalone '+name+' missing: pass its explicit path')
    return path.resolve()

def build(*,skip_native=False,ffmpeg=None,ytdlp=None,node=None,backend_directory=None,overlay_executable=None):
    if os.name!='nt':raise RuntimeError('The native Windows distribution must be built on Windows')
    if not skip_native:
        for script in ('build-backend.ps1','build-overlay.ps1'):
            subprocess.run(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(ROOT/'scripts'/script)],cwd=ROOT,check=True)
    native=PACKAGE/'_native';backend=Path(backend_directory).resolve() if backend_directory else native
    overlay=Path(overlay_executable).resolve() if overlay_executable else native/'overlay/hoi4-overlay.exe'
    required=[backend/'hoi4-backend.exe',backend/'hoi4-desktop.exe',backend/'hoi4companion_backend.dll',native/'hoi4companion_parser.dll',overlay,native/'cwtools/Companion.Cwtools.exe']
    for path in required:
        if not path.is_file():raise RuntimeError('Missing native application component: '+str(path))
    tools={'ffmpeg.exe':locate_tool('ffmpeg.exe',ffmpeg),'yt-dlp.exe':locate_tool('yt-dlp.exe',ytdlp)}
    node_path=Path(node).resolve(strict=True) if node else ROOT/'.local/windows-build-downloads/node-v24.15.0-win-x64/node.exe'
    if not node_path.is_file():raise ValueError('Pinned Node utility missing: pass --node (used only by YouTube acquisition)')
    tools['node.exe']=node_path
    version=tomllib.loads((ROOT/'pyproject.toml').read_text(encoding='utf8'))['project']['version']
    output=ROOT/f'dist/HOI4Companion-{version}-native-windows-x64.zip';output.parent.mkdir(exist_ok=True)
    # Only one short-lived staging directory; it is owned by this invocation
    # and is automatically removed after the resulting ZIP has been verified.
    with tempfile.TemporaryDirectory(prefix='hoi4-native-package-') as temporary:
        destination=Path(temporary)/'HOI4Companion';resources=destination/'resources';resources.mkdir(parents=True)
        shutil.copyfile(backend/'hoi4-desktop.exe',destination/'HOI4 Companion.exe')
        for folder in RESOURCE_FOLDERS:shutil.copytree(PACKAGE/folder,resources/folder,ignore=shutil.ignore_patterns('__pycache__','*.pyc','*.pdb'))
        for name in RESOURCE_FILES:shutil.copyfile(PACKAGE/name,resources/name)
        (resources/'guides').mkdir() # User-authored/imported guides are never bundled.
        target=resources/'_native';target.mkdir()
        for name in ('hoi4-backend.exe','hoi4companion_backend.dll'):shutil.copyfile(backend/name,target/name)
        shutil.copyfile(native/'hoi4companion_parser.dll',target/'hoi4companion_parser.dll')
        (target/'overlay').mkdir();shutil.copyfile(overlay,target/'overlay/hoi4-overlay.exe')
        (target/'overlay/provenance.json').write_text(json.dumps({'schema_version':1,'engine':'rust-native-hud','native_hud_protocol':1,'files':{'hoi4-overlay.exe':sha(overlay)}},indent=2)+'\n',encoding='utf8')
        shutil.copytree(native/'cwtools',target/'cwtools',ignore=shutil.ignore_patterns('*.pdb','*.log'))
        tools_target=target/'tools';tools_target.mkdir()
        tool_manifest={}
        for name,path in tools.items():
            shutil.copyfile(path,tools_target/name)
            result=subprocess.run([str(path),'-version' if name=='ffmpeg.exe' else '--version'],capture_output=True,text=True,encoding='utf8',errors='replace',timeout=20,creationflags=subprocess.CREATE_NO_WINDOW)
            if result.returncode:raise RuntimeError('Standalone utility failed its version check: '+name)
            tool_manifest[name]={'sha256':sha(path),'version':result.stdout.splitlines()[0],'on_demand':True}
            if name=='ffmpeg.exe':
                license_text=subprocess.run([str(path),'-L'],capture_output=True,text=True,encoding='utf8',errors='replace',timeout=20,creationflags=subprocess.CREATE_NO_WINDOW)
                (tools_target/'ffmpeg-license.txt').write_text(license_text.stdout+license_text.stderr,encoding='utf8')
        (tools_target/'yt-dlp-license.txt').write_text('yt-dlp standalone utility: https://github.com/yt-dlp/yt-dlp/blob/master/LICENSE\nThe main yt-dlp project is released into the public domain (Unlicense). Standalone builds include third-party components; see https://github.com/yt-dlp/yt-dlp#license.\n',encoding='utf8')
        if node_path.with_name('LICENSE').is_file():shutil.copyfile(node_path.with_name('LICENSE'),tools_target/'node-LICENSE.txt')
        (tools_target/'manifest.json').write_text(json.dumps(tool_manifest,indent=2)+'\n',encoding='utf8')
        for name in ('README.md','LICENSE','START HERE.txt'):shutil.copyfile(ROOT/name,destination/name)
        shutil.copytree(ROOT/'docs',destination/'docs')
        files={path.relative_to(destination).as_posix():sha(path) for path in sorted(destination.rglob('*')) if path.is_file()}
        forbidden=('python.exe','pythonw.exe','python3.dll','python313.dll','companion runtime.exe','.sqlite3','.pyc','.pdb','settings.json','.env','/guides/','/guide-videos/','/sprite-cache/')
        if any(any(word in name.lower() for word in forbidden) for name in files):raise RuntimeError('Forbidden application runtime or user data in native package')
        manifest={'schema_version':1,'version':version,'architecture':'rust-backend-rust-hud-javascript-ui','resident_python':False,'data_directory':'%LOCALAPPDATA%/HOI4Companion','git_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'working_tree_dirty':bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)),'files':files,'utilities':tool_manifest}
        (destination/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf8')
        with zipfile.ZipFile(output,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=6) as bundle:
            for path in sorted(destination.rglob('*')):
                if path.is_file():bundle.write(path,'HOI4Companion/'+path.relative_to(destination).as_posix())
        with zipfile.ZipFile(output) as bundle:
            if bundle.testzip() is not None:raise RuntimeError('Native distribution ZIP verification failed')
    result={'zip':str(output),'bytes':output.stat().st_size,'sha256':sha(output),'resident_python':False,'files':len(files)}
    output.with_suffix('.zip.sha256').write_text(result['sha256']+'  '+output.name+'\n',encoding='ascii')
    print(json.dumps(result,indent=2));return result

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--skip-native',action='store_true');parser.add_argument('--ffmpeg');parser.add_argument('--ytdlp');parser.add_argument('--node');parser.add_argument('--backend-directory');parser.add_argument('--overlay-executable')
    build(**vars(parser.parse_args()))
