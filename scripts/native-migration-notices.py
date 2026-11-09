"""Collect original registry notices for the added native migration crates."""
import os
from pathlib import Path
import tomllib
import json
import hashlib


def main():
    root=Path(__file__).resolve().parents[1]
    configured=os.environ.get('CARGO_HOME')
    # Checkout builds may use the isolated toolchain; CI normally uses Cargo's
    # default user directory. Explicit CARGO_HOME remains authoritative.
    homes=[Path(configured)] if configured else [root/'.local/toolchains/cargo',Path.home()/'.cargo']
    registries=[registry for home in homes for registry in (home/'registry/src').glob('*')]
    packages={}
    for name in ('guide-core','catalog-core','tools','storage','backend'):
        lock=tomllib.loads((root/'native'/name/'Cargo.lock').read_text(encoding='utf8'))
        for row in lock['package']:
            if row.get('source','').startswith('registry+'):
                packages[row['name'],row['version']]=row
    result=['Native guide/catalog/tool/storage dependency notices.','Original text from the locked crates.io sources.','']
    for name,version in sorted(packages):
        candidates=[folder/(name+'-'+version) for folder in registries]
        source=next((folder for folder in candidates if folder.is_dir()),None)
        if source is None:raise ValueError('Missing locked source: '+name+' '+version)
        licenses=sorted(path for path in source.glob('*') if path.is_file() and any(word in path.name.upper() for word in ('LICENSE','COPYING','NOTICE','UNLICENSE')))
        if not licenses:
            manifest=tomllib.loads((source/'Cargo.toml').read_text(encoding='utf8'))
            declared=manifest.get('package',{}).get('license-file')
            if declared:licenses=[source/declared]
        if not licenses:
            # Some monorepo crates omit their repository-level license when
            # published. Retain the exact version-pinned upstream text locally;
            # source distributions and offline CI must not fetch mutable URLs.
            override=root/'src/hoi4companion/licenses/native-source-notices'/(name+'-'+version)
            provenance=override/'provenance.json'
            if provenance.is_file():
                notice=json.loads(provenance.read_text(encoding='utf8'))
                if notice.get('crate')!=name or notice.get('version')!=version:raise ValueError('Notice identity differs: '+name)
                for filename,details in notice['files'].items():
                    if Path(filename).name!=filename:raise ValueError('Invalid notice filename')
                    path=override/filename
                    if hashlib.sha256(path.read_bytes()).hexdigest()!=details['sha256']:raise ValueError('Notice hash differs: '+name)
                    licenses.append(path)
        if not licenses:raise ValueError('Missing dependency notice: '+name+' '+version)
        result.extend([name+' '+version,'https://crates.io/crates/'+name+'/'+version,''])
        for path in licenses:result.extend(['Source: '+path.name,path.read_text(encoding='utf8'), ''])
        result.append('-'*40)
    result.extend(['SQLite bundled by libsqlite3-sys is in the public domain.',
        'https://sqlite.org/copyright.html',
        'The Unicode compatibility profile is generated from Python Unicode '+__import__('unicodedata').unidata_version+' data.',
        'Python and Unicode notices are retained with the application distribution.'])
    output=root/'src/hoi4companion/licenses/native-migration-dependencies.txt'
    output.write_text('\n'.join(result)+'\n',encoding='utf8');print(output)


if __name__=='__main__':main()
