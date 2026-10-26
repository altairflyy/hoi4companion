"""Windows desktop entry and fixed worker dispatch for the self-contained build."""
import argparse
from contextlib import contextmanager
import ctypes
import json
from pathlib import Path
import runpy
import socket
import subprocess
import sys
import time
import traceback
from urllib.request import urlopen
import webbrowser

from .paths import data_directory, module_command

WORKERS={'hoi4companion','hoi4companion.overlay_bridge','hoi4companion.overlay_native','yt_dlp'}
FLAGS=getattr(subprocess,'CREATE_NO_WINDOW',0)


def dispatch_worker(args):
    if len(args)<2 or args[0]!='-m' or args[1] not in WORKERS:
        raise ValueError('Worker module is not allowed')
    original=sys.argv
    try:
        sys.argv=[args[1],*args[2:]]
        return runpy.run_module(args[1],run_name='__main__',alter_sys=True)
    finally:sys.argv=original


def probe(base):
    try:
        with urlopen(base+'/api/config',timeout=3) as response:value=json.load(response)
        return isinstance(value,dict) and value.get('application')=='hoi4companion'
    except (OSError,ValueError):return False


def port_open(port):
    with socket.socket() as connection:
        connection.settimeout(1)
        return connection.connect_ex(('127.0.0.1',port))==0


@contextmanager
def launch_mutex(port):
    if sys.platform!='win32':yield;return
    kernel=ctypes.WinDLL('kernel32',use_last_error=True)
    kernel.CreateMutexW.argtypes=[ctypes.c_void_p,ctypes.c_bool,ctypes.c_wchar_p]
    kernel.CreateMutexW.restype=ctypes.c_void_p
    kernel.CloseHandle.argtypes=[ctypes.c_void_p]
    handle=kernel.CreateMutexW(None,False,'Local\\HOI4CompanionDesktop'+str(port))
    if not handle:raise OSError('Could not coordinate startup')
    try:
        if ctypes.get_last_error()==183:raise RuntimeError('Startup is already in progress: wait for the first window.')
        yield
    finally:kernel.CloseHandle(handle)


def launch(*,port=8766,browser=True,hud=True):
    if type(port) is not int or not 1024<=port<=65535:raise ValueError('Invalid local port')
    directory=data_directory();directory.mkdir(parents=True,exist_ok=True)
    base='http://127.0.0.1:'+str(port)
    with launch_mutex(port):
        if not probe(base):
            if port_open(port):raise RuntimeError(f'Porta {port} occupata da un altro servizio. Nessun processo è stato chiuso.')
            with (directory/'server.log').open('a',encoding='utf8') as log:
                server=subprocess.Popen(module_command('hoi4companion','--no-browser','--port',port),
                    stdout=log,stderr=log,creationflags=FLAGS)
            deadline=time.monotonic()+240
            while not probe(base):
                if server.poll() is not None:raise RuntimeError('Startup failed: see '+str(directory/'server.log'))
                if time.monotonic()>deadline:raise RuntimeError('Loading is still in progress: see '+str(directory/'server.log'))
                time.sleep(.5)
        if hud:
            from .overlay_native import command, interface_language
            with (directory/'hud.log').open('a',encoding='utf8') as log:
                process=subprocess.Popen(command(port=port,start=True,ui_language=interface_language(port)),stdout=log,stderr=log,creationflags=FLAGS)
            time.sleep(.5)
            if process.poll() not in (None,0):raise RuntimeError('HUD did not start: see '+str(directory/'hud.log'))
        if browser:webbrowser.open(base+'/walkthrough')


def self_check(output):
    """Offline deployment proof, independent of any installed game or save."""
    from .jomini_parser import backend_info,parse
    from .cwtools_engine import _verify,HOST_ROOT,RULES,analyze
    from .guide_media import ffmpeg_path,command,media_info
    from .overlay_native import native_executable
    from .paths import bundled_node
    import cv2,numpy,yt_dlp,yt_dlp_ejs
    from tempfile import TemporaryDirectory
    assert parse('test={item=1 item=2}').blocks('test')[0].all('item')==['1','2']
    _verify(HOST_ROOT);_verify(RULES)
    ffmpeg=ffmpeg_path();node=bundled_node()
    with TemporaryDirectory(prefix='hoi4-distribution-') as temporary:
        script=Path(temporary)/'event.txt';script.write_text('country_event={id=example.1 title=example.1.t option={name=example.1.a}}',encoding='utf8')
        analysis=analyze({'events/event.txt':str(script)})
        assert analysis['engine']=='cwtools'
        video=Path(temporary)/'sample.mp4'
        command([ffmpeg,'-hide_banner','-nostdin','-loglevel','error','-f','lavfi','-i',
            'color=c=black:s=160x90:r=15','-t','1','-c:v','libx265','-x265-params',
            'pools=1:frame-threads=1:log-level=error',video],timeout=45)
        media=media_info(ffmpeg,video)
    result={'parser':backend_info(),'cwtools_manifest_verified':True,'rules_manifest_verified':True,
        'hud':str(native_executable()),'ffmpeg_version':command([ffmpeg,'-version']).splitlines()[0],
        'opencv':cv2.__version__,'numpy':numpy.__version__,'yt_dlp':yt_dlp.version.__version__,
        'cwtools_engine_executed':analysis['engine'],'hevc_encode_verified':media['height']==90,
        'node_version':command([node,'--version']).strip() if node else None,'data_directory':str(data_directory())}
    Path(output).write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf8')


def main():
    args=sys.argv[1:]
    if args[:1]==['-m']:return dispatch_worker(args)
    parser=argparse.ArgumentParser(description='Start HOI4 Companion and HUD on Windows')
    parser.add_argument('--port',type=int,default=8766)
    parser.add_argument('--no-browser',action='store_true')
    parser.add_argument('--no-hud',action='store_true')
    parser.add_argument('--self-check',type=Path)
    options=parser.parse_args(args)
    if options.self_check:return self_check(options.self_check)
    try:launch(port=options.port,browser=not options.no_browser,hud=not options.no_hud)
    except Exception as exc:
        directory=data_directory();directory.mkdir(parents=True,exist_ok=True)
        (directory/'launcher-error.log').write_text(traceback.format_exc(),encoding='utf8')
        if sys.platform=='win32':ctypes.windll.user32.MessageBoxW(None,str(exc),'HOI4 Companion',0x10)
        raise


if __name__=='__main__':main()
