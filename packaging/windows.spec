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
datas.append((str(package/'guide_language.json'),'hoi4companion'))
datas.append((str(package/'live-profiles-v1.json'),'hoi4companion'))
datas.append((str(package/'design-profile-v1.json'),'hoi4companion'))
datas.append((str(package/'country-functions-v1.json'),'hoi4companion'))
datas.append((str(package/'translations.json'),'hoi4companion'))
for folder in ('cwtools','overlay'):
    # Keep provenance manifests and their bytes unchanged.
    datas.append((str(native/folder),'hoi4companion/_native/'+folder))
parser=native/'hoi4companion_parser.dll'
if not parser.is_file():parser=next(native.glob('hoi4companion_parser*.pyd'))
binaries.append((str(parser),'hoi4companion/_native'))
observer=native/'hoi4companion_observer.dll'
if not observer.is_file():observer=next(native.glob('hoi4companion_observer*.pyd'))
binaries.append((str(observer),'hoi4companion/_native'))
guide_core=native/'hoi4companion_guide_core.dll'
if not guide_core.is_file():guide_core=next(native.glob('hoi4companion_guide_core*.pyd'))
binaries.append((str(guide_core),'hoi4companion/_native'))
catalog_core=native/'hoi4companion_catalog_core.dll'
if not catalog_core.is_file():catalog_core=next(native.glob('hoi4companion_catalog_core*.pyd'))
binaries.append((str(catalog_core),'hoi4companion/_native'))
tools=native/'hoi4companion_tools.dll'
if not tools.is_file():tools=next(native.glob('hoi4companion_tools*.pyd'))
binaries.append((str(tools),'hoi4companion/_native'))
storage=native/'hoi4companion_storage.dll'
if not storage.is_file():storage=next(native.glob('hoi4companion_storage*.pyd'))
binaries.append((str(storage),'hoi4companion/_native'))
backend=native/'hoi4companion_backend.dll'
if not backend.is_file():backend=next(native.glob('hoi4companion_backend*.pyd'))
binaries.append((str(backend),'hoi4companion/_native'))
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
