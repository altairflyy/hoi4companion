"""Build and package the external Windows overlay; never starts it or HOI4."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]


def build(destination):
    if sys.platform!='win32':
        raise RuntimeError('Overlay Rust attualmente verificato su Windows x64')
    local=ROOT/'.local/toolchains/cargo/bin/cargo.exe'
    cargo=str(local) if local.is_file() else shutil.which('cargo')
    if not cargo:raise RuntimeError('Compilazione overlay: Rust e build tools MSVC richiesti')
    env=os.environ.copy()
    if local.is_file():
        env['CARGO_HOME']=str(ROOT/'.local/toolchains/cargo')
        env['RUSTUP_HOME']=str(ROOT/'.local/toolchains/rustup')
    target=ROOT/'.local/rust-overlay-package-target'
    env['CARGO_TARGET_DIR']=str(target)
    subprocess.run([cargo,'build','--release','--locked','--manifest-path',
                    str(ROOT/'native/overlay/Cargo.toml')],env=env,cwd=ROOT,check=True)
    destination=Path(destination).resolve();destination.mkdir(parents=True,exist_ok=True)
    executable=destination/'hoi4-overlay.exe'
    shutil.copyfile(target/'release/hoi4-overlay.exe',executable)
    manifest={'schema_version':1,'engine':'native-gui-direct2d','bridge_schema':1,
              'native_panels':['focus','research','research_tab'],'visual_fallback':'wgc-winrtocr',
              'files':{'hoi4-overlay.exe':hashlib.sha256(executable.read_bytes()).hexdigest()}}
    (destination/'provenance.json').write_text(json.dumps(manifest,indent=2),encoding='utf8')
    return destination


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,default=ROOT/'src/hoi4companion/_native/overlay')
    print(build(parser.parse_args().output))
