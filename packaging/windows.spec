# Two entry points share the bundled modules: GUI launcher and pipe-capable worker.
from pathlib import Path
import os
from PyInstaller.utils.hooks import collect_all,copy_metadata

root=Path(SPECPATH).parent
package=root/'src/hoi4companion'
native=Path(os.environ['HOI4_BUILD_NATIVE'])
datas=[];binaries=[];hidden=[]
for folder in ('web','guides','examples','licenses','rules'):
    datas.append((str(package/folder),'hoi4companion/'+folder))
datas.append((str(package/'guide-import-prompt.md'),'hoi4companion'))
for folder in ('cwtools','overlay'):
    # Keep provenance manifests and their bytes unchanged.
    datas.append((str(native/folder),'hoi4companion/_native/'+folder))
parser=native/'hoi4companion_parser.dll'
if not parser.is_file():parser=next(native.glob('hoi4companion_parser*.pyd'))
binaries.append((str(parser),'hoi4companion/_native'))
datas.append((os.environ['HOI4_BUILD_NODE'],'hoi4companion/_native/node'))
for name in ('yt_dlp','yt_dlp_ejs','imageio_ffmpeg'):
    data,bins,imports=collect_all(name)
    datas+=data;binaries+=bins;hidden+=imports
for name in ('yt-dlp','yt-dlp-ejs','imageio-ffmpeg'):
    datas+=copy_metadata(name)
hidden += ['cv2','numpy']
hidden += ['hoi4companion.'+p.relative_to(package).with_suffix('').as_posix().replace('/','.')
           for p in package.rglob('*.py') if '_native' not in p.parts and p.name!='__init__.py']
a=Analysis([str(root/'packaging/portable_entry.py')],pathex=[str(root/'src')],
    binaries=binaries,datas=datas,hiddenimports=hidden,excludes=['tkinter','pytest','IPython'],
    noarchive=False,optimize=0)
pyz=PYZ(a.pure)
worker=EXE(pyz,a.scripts,[],exclude_binaries=True,name='Companion Runtime',
    console=True,debug=False,strip=False,upx=False)
launcher=EXE(pyz,a.scripts,[],exclude_binaries=True,name='HOI4 Companion',
    console=False,debug=False,strip=False,upx=False)
collect=COLLECT(worker,launcher,a.binaries,a.datas,strip=False,upx=False,name='HOI4Companion')
