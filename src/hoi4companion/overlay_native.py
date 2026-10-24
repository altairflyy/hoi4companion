"""Launch the native Rust overlay; the original backend remains explicit only."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
from urllib.request import urlopen

ROOT=Path(__file__).resolve().parent


def native_executable():
    override=os.environ.get('HOI4_OVERLAY_NATIVE')
    candidate=Path(override).expanduser() if override else ROOT/'_native'/'overlay'/'hoi4-overlay.exe'
    if not candidate.is_file():
        raise RuntimeError('Overlay Rust non installato. Installa il pacchetto completo oppure '
                           'compila native/overlay con scripts/build-overlay.ps1.')
    return candidate.resolve()


def _console_python():
    executable=Path(sys.executable)
    if executable.name.casefold().startswith('pythonw'):
        candidate=executable.with_name('python'+executable.name[len('pythonw'):])
        if candidate.is_file():return str(candidate)
    return str(executable)


def command(*,port=8766,guide='',start=False,proof=None):
    if type(port) is not int or not 1024<=port<=65535:raise ValueError('Porta locale non valida')
    if not isinstance(guide,str):raise ValueError('ID guida non valido')
    result=[str(native_executable()),'--port',str(port),'--bridge-python',_console_python()]
    if guide:result+=['--guide',guide]
    if start:result.append('--start')
    if proof:result+=['--proof',str(Path(proof).expanduser().absolute())]
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port',type=int,default=8766)
    parser.add_argument('--guide',default='')
    parser.add_argument('--start',action='store_true')
    parser.add_argument('--proof',type=Path)
    parser.add_argument('--list-guides',action='store_true')
    args=parser.parse_args()
    if not 1024<=args.port<=65535:parser.error('Porta locale non valida')
    if args.list_guides:
        with urlopen(f'http://127.0.0.1:{args.port}/api/guides',timeout=10) as response:
            guides=json.load(response)['guides']
        for guide in guides:print(guide['id']+'\t'+guide['title'])
        return
    if os.name!='nt':parser.exit(1,'Overlay disponibile solo su Windows\n')
    try:
        args=command(port=args.port,guide=args.guide,start=args.start,proof=args.proof)
        flags=getattr(subprocess,'CREATE_NO_WINDOW',0)
        result=subprocess.run(args,creationflags=flags,check=False)
        raise SystemExit(result.returncode)
    except (OSError,RuntimeError,ValueError) as exc:parser.exit(1,str(exc)+'\n')


if __name__=='__main__':main()
