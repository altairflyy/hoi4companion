"""Sprite originali locali: .gfx -> texture -> PNG, senza copiare asset nel sorgente."""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
from io import BytesIO
from pathlib import Path, PurePosixPath
import struct
from threading import RLock

from .clausewitz import Node
from .jomini_parser import parse_file
from .installation import collect_files

try:
    from PIL import Image, UnidentifiedImageError
except ImportError:
    Image = None
    UnidentifiedImageError = OSError


@dataclass(frozen=True)
class Sprite:
    name: str
    texture: str
    frames: int = 1


def _fields(node: Node) -> dict:
    return {k.casefold(): v for k, v in node.entries}


def _definitions(node: Node):
    for key, value in node.entries:
        if not isinstance(value, Node):
            continue
        if key.casefold() in ('spritetype', 'frameanimatedspritetype', 'corneredtilespritetype'):
            fields = _fields(value)
            name, texture = fields.get('name'), fields.get('texturefile')
            if isinstance(name, str) and isinstance(texture, str):
                try:
                    frames = int(fields.get('noofframes', '1'))
                except (ValueError, TypeError):
                    continue
                if 1 <= frames <= 256:
                    yield Sprite(name, texture, frames)
        else:
            yield from _definitions(value)


class SpriteLibrary:
    @classmethod
    def from_catalog(cls,owner,cache):
        result=cls.__new__(cls);result._catalog_owner=owner
        info=owner.query('assets_info');result.roots=list(map(Path,info['roots']));result.cache=cache
        result.sprites={key:Sprite(**value) for key,value in info['sprites'].items()}
        result.warnings=info['warnings'];result.failed=set();result.assets={};result.resolved={};result.replacements={};result.lock=RLock()
        return result

    def _native_request(self,operation,**args):
        owner=getattr(self,'_catalog_owner',None)
        if owner is not None:return owner.query('assets_'+operation,**args)
        from .backend_core import _request
        return _request(0,'assets_'+operation,handle=self._handle,**args)

    def __init__(self,game:Path,extra_roots:list[Path],cache:Path):
        from .backend_core import _request
        import weakref
        self._handle=_request(0,'assets_open',game=str(game),roots=list(map(str,extra_roots)),cache=str(cache))
        self._finalizer=weakref.finalize(self,_request,0,'assets_release',handle=self._handle)
        info=_request(0,'assets_info',handle=self._handle)
        self.roots=list(map(Path,info['roots']));self.cache=cache
        self.sprites={key:Sprite(**value) for key,value in info['sprites'].items()}
        self.warnings=info['warnings'];self.failed=set();self.assets={};self.resolved={};self.replacements={};self.lock=RLock()

    def _texture(self,relative:str)->Path|None:
        from .backend_core import _request
        value=self._native_request('texture',texture=relative)
        return Path(value) if value is not None else None

    def resolve(self,name:str,frame:int=1)->dict|None:
        from .backend_core import _request
        with self.lock:
            key=(name,frame)
            if key in self.resolved:return self.resolved[key]
            if type(frame) is not int or frame<0:return None
            value=self._native_request('resolve',name=name,frame=frame)
            self.resolved[key]=value['sprite']
            if value['sprite']:
                asset=value['asset'];token=value['sprite']['url'].rsplit('/',1)[-1][:-4]
                self.assets[token]=(Path(asset['path']),asset['frames'],asset['frame'])
            return value['sprite']

    def png(self,token:str)->bytes|None:
        from .backend_core import _request
        with self.lock:
            value=self._native_request('png',token=token)
            if value is None:
                if token in self.assets:self.failed.add(token)
                return None
            return Path(value).read_bytes()

    @staticmethod
    def _decode(raw: bytes):
        # DDS DX10 BGRA sRGB (DXGI 91) compare in tre icone CZE installate.
        if len(raw) >= 148 and raw[:4] == b'DDS ' and raw[84:88] == b'DX10':
            height, width, pitch = struct.unpack_from('<III', raw, 12)
            fmt, dimension, misc, count = struct.unpack_from('<IIII', raw, 128)
            if fmt in (87, 91) and dimension == 3 and count == 1 and not misc & 4:
                if not 0 < width * height <= 8_000_000:
                    raise ValueError('Dimensioni DDS non supportate')
                stride = pitch if pitch >= width * 4 else width * 4
                if stride * height > len(raw) - 148:
                    raise ValueError('DDS troncato')
                return Image.frombytes('RGBA', (width, height), raw[148:], 'raw', 'BGRA', stride, 1)
        image = Image.open(BytesIO(raw))
        if image.width * image.height > 8_000_000:
            image.close()
            raise ValueError('Texture troppo grande')
        try:
            return image.convert('RGBA')
        finally:
            image.close()

    def inline(self, texts: list[str]) -> dict:
        import re
        result = {}
        for text in texts:
            for match in re.finditer(r'£([A-Za-z0-9_]+)(?:\|(\d+))?£?', text):
                token = match.group(0)
                name = match.group(1)
                frame = int(match.group(2) or 1)
                # Il frame 0 nei testi non seleziona un frame noto: resta testo.
                if frame < 1:
                    continue
                name = name if name.startswith('GFX_') else 'GFX_' + name
                if sprite := self.resolve(name, frame):
                    result[token] = sprite
        return result

    def theme(self) -> dict:
        names = {'locked': 'GFX_focus_unavailable', 'unknown': 'GFX_focus_unavailable',
                 'verify': 'GFX_focus_unavailable', 'excluded': 'GFX_focus_unavailable',
                 'ready': 'GFX_focus_can_start', 'active': 'GFX_focus_can_start',
                 'completed': 'GFX_focus_completed', 'exclusive': 'GFX_focus_link_exclusive'}
        return {key: value for key, name in names.items() if (value := self.resolve(name))}
