"""Content-checked encoding cache, independent of video segmentation."""
import hashlib
import json
from pathlib import Path
import re


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream,'sha256').hexdigest()


class MasterCache:
    def __init__(self,root,profile):
        self.root=Path(root).resolve();self.profile=profile

    def identity(self,source):
        identity={'source_sha256':digest(source),'profile':self.profile}
        return hashlib.sha256(json.dumps(identity,sort_keys=True).encode()).hexdigest(),identity

    def path(self,key):
        if not re.fullmatch('[0-9a-f]{64}',key):raise ValueError('Chiave cache video non valida')
        return self.root/(key+'.mp4')

    def load(self,key):
        path=self.path(key)
        try:
            metadata=json.loads(path.with_suffix('.json').read_text(encoding='utf8'))
            if (metadata['profile']!=self.profile or not path.is_file() or path.is_symlink()
                    or path.stat().st_size!=metadata['bytes'] or digest(path)!=metadata['sha256']):return None
            return path,metadata
        except (OSError,ValueError,KeyError,TypeError):return None

    def save(self,key,identity,encoded,details):
        self.root.mkdir(parents=True,exist_ok=True)
        path=self.path(key)
        metadata={**details,**identity,'bytes':Path(encoded).stat().st_size,'sha256':digest(encoded)}
        Path(encoded).replace(path)
        temporary=path.with_suffix('.json.tmp');temporary.write_text(json.dumps(metadata,ensure_ascii=False,indent=2),encoding='utf8')
        temporary.replace(path.with_suffix('.json'))
        return path,metadata

    def cleanup(self,url,keep_key):
        removed=0
        if not self.root.is_dir():return removed
        for record in self.root.glob('*.json'):
            if record.stem==keep_key or not re.fullmatch('[0-9a-f]{64}',record.stem) or record.is_symlink():continue
            try:
                if json.loads(record.read_text(encoding='utf8')).get('source_url')!=url:continue
                path=self.path(record.stem)
                if path.is_file() and not path.is_symlink():removed+=path.stat().st_size;path.unlink()
                removed+=record.stat().st_size;record.unlink()
            except (OSError,ValueError,KeyError):continue
        return removed
