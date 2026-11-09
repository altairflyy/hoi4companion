"""User-supplied design references. Never installs anything into the game."""
import hashlib
from pathlib import Path
import re
import unicodedata



def name_key(value):
    # Plus denotes a distinct model. Hyphens/spaces are editorial separators.
    return ''.join(c for c in unicodedata.normalize('NFKD',value).casefold() if c.isalnum() or c=='+')


def base_name(value):
    return value.split('(',1)[0].strip()



def image_metadata(path):
    """Classify declared archive folders, never guess a model's military role."""
    patterns=(('templates',r'(?:division\s+)?templates?'),('armor',r'(?:armor|armour|tank)(?:\s+designs?)?'),
              ('air',r'(?:air|aircraft|aviation)(?:\s+designs?)?'),('navy',r'(?:navy|naval|ship)(?:\s+designs?)?'),
              ('multiplayer',r'multiplayer'))
    parts=path.parent.parts;category='other';start=0
    for i in range(len(parts)-1,-1,-1):
        label=re.sub(r'\s*\([^)]*\)\s*',' ',parts[i]).strip()
        match=next((key for key,pattern in patterns if re.fullmatch(pattern,label,re.I)),None)
        if match:category=match;start=i;break
    tail=' '.join(parts[start:])
    return {'category':category,'tags':[tag for tag in ('situational','outdated') if re.search(r'\b'+tag+r'\b',tail,re.I)]}


class ReferenceLibrary:
    def __init__(self,root):
        self.root=Path(root)

    def manifest(self):
        from .backend_core import _request
        return _request(0,'reference_library_manifest',root=str(self.root))

    def import_archive(self,stream):
        from .backend_core import _request
        from .word_guides import _native_source
        with _native_source(stream) as path:
            return _request(0,'reference_library_import',root=str(self.root),path=str(path))

    def image(self,identifier):
        from .backend_core import _request
        result=_request(0,'reference_library_image_path',root=str(self.root),id=identifier)
        raw=Path(result['path']).read_bytes()
        if len(raw)>16_000_000 or hashlib.sha256(raw).hexdigest()!=result['sha256']:
            raise ValueError('Reference image checksum mismatch')
        return raw,result['mime']


def design_matches(query,designs,upgrades=None):
    from .backend_core import _request
    return _request(0,'reference_matches',query=query,designs=designs,upgrades=upgrades)


def bindings(guide,catalog):
    from .backend_core import _request
    from .resolution import catalog_index
    index=catalog_index(catalog)
    return _request(0,'reference_bindings',guide=guide,library=index.reference_designs,**index.native_import_args())


def selector(query,catalog,declared=None):
    from .backend_core import _request
    from .resolution import catalog_index
    index=catalog_index(catalog)
    return _request(0,'reference_selector',query=query,library=index.reference_designs,declared=declared,**index.native_import_args())


def attach_images(guide,catalog):
    from .backend_core import _request,_publish_in_place
    result=_request(0,'reference_attach_images',guide=guide,library=getattr(catalog,'reference_designs',{}))
    _publish_in_place(guide,result['guide'])
    return result['changes']
