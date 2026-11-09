"""Read-only extraction of the shipped technology GUI templates and textures.

Positions refer to the folder's content origin, excluding its scrolling viewport.
The GUI's placeholder background is replaced by a named folder state sprite only
when that sprite exists in the parsed GFX inventory. No engine state is inferred.
"""
from __future__ import annotations

import math
from pathlib import Path, PurePosixPath
import struct

from .clausewitz import Node
from .jomini_parser import parse_file


STATES = ('unavailable', 'available', 'researched', 'currently_researching')


def _field(node, key, default=''):
    if not isinstance(node, Node):
        return default
    return next((v for k, v in node.entries if k.casefold() == key.casefold()), default)


def _number(node, key, default=None):
    if not isinstance(node, Node):
        return default
    value = _field(node, key)
    try:
        result = float(value)
        return result if math.isfinite(result) else default
    except (TypeError, ValueError):
        return default


def _point(node, field='position', keys=('x', 'y')):
    value = _field(node, field)
    return {key: _number(value, key) for key in keys}


def _walk(node):
    for kind, value in node.entries:
        if isinstance(value, Node):
            yield kind.casefold(), value
            yield from _walk(value)


def _dimensions(path, frames):
    """Read only portable texture headers; reject unsupported/oversized data."""
    try:
        with path.open('rb') as stream:
            data = stream.read(128)
        if data[:4] == b'DDS ' and len(data) >= 128:
            height, width = struct.unpack_from('<II', data, 12)
        elif data[:8] == b'\x89PNG\r\n\x1a\n' and len(data) >= 24:
            width, height = struct.unpack_from('>II', data, 16)
        elif path.suffix.casefold() == '.tga' and len(data) >= 18:
            width, height = struct.unpack_from('<HH', data, 12)
        else:
            return None
        if not 0 < width * height <= 8_000_000 or not 1 <= frames <= 256 or width % frames:
            return None
        return {'width': width // frames, 'height': height}
    except (OSError, ValueError, struct.error):
        return None


def _texture_path(gfx_file, texture):
    relative = PurePosixPath(texture.replace('\\', '/'))
    if relative.is_absolute() or '..' in relative.parts or ':' in texture:
        return None
    # Installed files live under <game>/interface; mod files can use the same
    # structure. Find that explicit ancestor rather than searching the disk.
    interface = next((p for p in gfx_file.parents if p.name.casefold() == 'interface'), None)
    if interface is None:
        return None
    root = interface.parent.resolve()
    for candidate in (relative, relative.with_suffix('.dds')):
        path = (root / str(candidate)).resolve()
        if path.is_relative_to(root) and path.is_file():
            return path
    return None


def load_technology_visuals(gui_files,gfx_files,sprite_library=None):
    from .backend_core import _request
    handle=getattr(sprite_library,'_handle',None)
    if sprite_library is not None and handle is None:raise ValueError('Native sprite generation required')
    return _request(0,'source_technology_visuals',gui_files=list(map(str,gui_files)),gfx_files=list(map(str,gfx_files)),assets_handle=handle)['value']
