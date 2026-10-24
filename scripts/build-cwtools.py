"""Build the pinned read-only CWTools host, including its own .NET runtime."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET

ROOT=Path(__file__).resolve().parents[1]
REVISION='b377453dee803f9258be92cfc49896d09039702d'


def build(destination):
    if sys.platform!='win32' or os.environ.get('PROCESSOR_ARCHITECTURE','').lower() not in ('amd64','x86_64'):
        raise RuntimeError('Il pacchetto CWTools è attualmente verificato per Windows x64')
    local=ROOT/'.local/toolchains/dotnet/dotnet.exe'
    dotnet=str(local) if local.is_file() else shutil.which('dotnet')
    if not dotnet:raise RuntimeError('Compilazione CWTools: installa .NET SDK 9')
    provenance=json.loads((ROOT/'native/cwtools/vendor-provenance.json').read_text(encoding='utf-8'))
    for name,digest in provenance['files'].items():
        if hashlib.sha256((ROOT/'native/cwtools/vendor'/name).read_bytes()).hexdigest()!=digest:
            raise RuntimeError('Sorgente CWTools modificato rispetto alla revisione inclusa: '+name)
    destination=Path(destination).resolve();destination.mkdir(parents=True,exist_ok=True)
    env=os.environ.copy();env['DOTNET_CLI_TELEMETRY_OPTOUT']='1';env['DOTNET_SKIP_FIRST_TIME_EXPERIENCE']='1'
    env['NUGET_PACKAGES']=str(ROOT/'.local/nuget-packages')
    # No runtime network or downloads: restore occurs only while building.
    subprocess.run([dotnet,'publish',str(ROOT/'native/cwtools/Companion.Cwtools.fsproj'),
                    '-c','Release','-r','win-x64','--self-contained','true','--nologo',
                    '-p:RestoreLockedMode=true','-o',str(destination)],env=env,cwd=ROOT/'native/cwtools',check=True)
    shutil.copyfile(ROOT/'native/cwtools/vendor/LICENSE',destination/'CWTools.LICENSE.txt')
    packages=ROOT/'.local/nuget-packages';notices=[]
    assets=json.loads((ROOT/'native/cwtools/obj/project.assets.json').read_text(encoding='utf-8'))
    for name,meta in sorted(assets['libraries'].items()):
        if meta['type']!='package':continue
        nuspec=next((packages/meta['path']).glob('*.nuspec'),None)
        notice={'package':name}
        if nuspec:
            document=ET.parse(nuspec).getroot()
            for element in document.iter():
                field=element.tag.rsplit('}',1)[-1]
                if field in ('license','licenseUrl','projectUrl','copyright') and element.text:
                    notice[field]=element.text
            for element in document.iter():
                if element.tag.rsplit('}',1)[-1]=='license' and element.get('type')=='file':
                    license_file=packages/meta['path']/element.text
                    if license_file.is_file():
                        target=name.replace('/','-')+'-LICENSE.txt'
                        shutil.copyfile(license_file,destination/target);notice['bundled_license']=target
        notices.append(notice)
    (destination/'THIRD-PARTY-NOTICES.json').write_text(json.dumps(notices,indent=2),encoding='utf-8')
    manifest={'schema_version':1,'repository':'https://github.com/cwtools/cwtools',
              'revision':REVISION,'runtime':'self-contained win-x64 .NET 9','parser_compatibility':1,'reviewed_recovery':1,'files':{}}
    for file in sorted(destination.rglob('*')):
        if file.is_file() and file.name!='provenance.json':
            manifest['files'][file.relative_to(destination).as_posix()]=hashlib.sha256(file.read_bytes()).hexdigest()
    (destination/'provenance.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
    return destination


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=ROOT/'src/hoi4companion/_native/cwtools')
    print(build(parser.parse_args().output))
