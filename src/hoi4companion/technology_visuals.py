"""Read-only extraction of the shipped technology GUI templates and textures.

Positions refer to the folder's content origin, excluding its scrolling viewport.
The GUI's placeholder background is replaced by a named folder state sprite only
when that sprite exists in the parsed GFX inventory. No engine state is inferred.
"""
from __future__ import annotations

import math
from pathlib import Path, PurePosixPath
import struct

from .clausewitz import Node, parse


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


def load_technology_visuals(gui_files, gfx_files, sprite_library=None):
    """Return templates, per-folder decoration layers and texture evidence.

    Optional SpriteLibrary adds local HTTP URLs. Header dimensions are available
    without PIL or decoding/caching complete background images.
    """
    result = {'templates': {}, 'folders': {}, 'warnings': [], 'basis': 'installed_gui_gfx_texture_headers'}
    sprites = {}
    for file in map(Path, gfx_files):
        try:
            root = parse(file.read_text(encoding='utf-8-sig'))
            for kind, node in _walk(root):
                if kind not in ('spritetype', 'frameanimatedspritetype', 'corneredtilespritetype'):
                    continue
                name, texture = _field(node, 'name'), _field(node, 'texturefile')
                if not isinstance(name, str) or not isinstance(texture, str) or not name or not texture:
                    continue
                frames = int(_field(node, 'noofframes', '1'))
                if not 1 <= frames <= 256:
                    continue
                path = _texture_path(file, texture)
                asset = {'name': name, 'sprite_name': name, 'texture': texture, 'frames': frames,
                         'frame': 1, 'source': str(file), 'kind': kind}
                size = _dimensions(path, frames) if path else None
                if size:
                    asset.update(size)
                if kind == 'corneredtilespritetype':
                    asset['tile'] = _field(node, 'tilingcenter') == 'yes'
                    asset['border'] = _point(node, 'bordersize')
                sprites[name] = asset
        except (OSError, ValueError, UnicodeError, RecursionError) as exc:
            result['warnings'].append(f'Technology graphics not readable ({file.name}): {exc}')

    def asset(name):
        value = dict(sprites[name]) if name in sprites else None
        if value and sprite_library is not None:
            resolved = sprite_library.resolve(name, frame=1)
            if resolved:
                value.update(resolved)
        return value

    def layer_walk(node, x=0.0, y=0.0, parent_size=None):
        for kind, child in node.entries:
            if not isinstance(child, Node):
                continue
            position = _point(child)
            # Keep a literal percentage origin explicit instead of inventing it.
            if any(v is None and _field(_field(child, 'position', Node()), k) for k, v in position.items()):
                continue
            cx, cy = x + (position['x'] or 0), y + (position['y'] or 0)
            sprite = _field(child, 'spritetype') or _field(child, 'quadtexturesprite')
            name = _field(child, 'name')
            decoration = (isinstance(sprite, str) and
                          ('techtree_bg' in sprite or sprite in ('GFX_techtree_stripes', 'GFX_wonderweapons_bg', 'GFX_tiled_research_bg')))
            if decoration and _field(child, 'hide') != 'yes':
                value = asset(sprite)
                if value:
                    layer = {'name': name, 'x': cx, 'y': cy, 'sprite_name': sprite, 'sprite': value,
                             'repeat': bool(value.get('tile'))}
                    if parent_size and value.get('tile'):
                        layer.update({k: v for k, v in parent_size.items() if v is not None})
                    else:
                        layer.update({k: value[k] for k in ('width', 'height') if k in value})
                    yield layer
            if kind.casefold() == 'containerwindowtype':
                size = _field(child, 'size', Node())
                minimum = _field(size, 'min', Node())
                resolved = {}
                for key in ('width', 'height'):
                    reference = (parent_size or {}).get(key) or {'width': 2522, 'height': 1276}[key]
                    def metric(block):
                        raw = str(_field(block, key))
                        try:
                            value = float(raw.rstrip('%'))
                            return value * reference / 100 if raw.endswith('%') else value
                        except ValueError:
                            return 0
                    resolved[key] = max(metric(size), metric(minimum)) or reference
                yield from layer_walk(child, cx, cy, resolved)

    for file in map(Path, gui_files):
        try:
            root = parse(file.read_text(encoding='utf-8-sig'))
            for kind, node in _walk(root):
                name = _field(node, 'name')
                if kind != 'containerwindowtype' or not isinstance(name, str):
                    continue
                if name.startswith('techtree_') and name.endswith('_item'):
                    suffix = name[len('techtree_'):-len('_item')]
                    small = suffix.endswith('_small')
                    folder = suffix[:-6] if small else suffix
                    # Exclude line/XOR widgets; technology item templates have an Icon.
                    icon = next((v for k, v in node.entries if isinstance(v, Node) and
                                 k.casefold() == 'icontype' and _field(v, 'name') == 'Icon'), None)
                    if icon is None:
                        continue
                    text = next((v for k, v in node.entries if isinstance(v, Node) and
                                 k.casefold() == 'instanttextboxtype' and _field(v, 'name') == 'Name'), None)
                    prefix = f'GFX_technology_{folder}_' + ('small_' if small else '')
                    backgrounds, specials = {}, {}
                    for state in STATES:
                        specific, generic = prefix + state + '_item_bg', 'GFX_technology_' + state + '_item_bg'
                        value = asset(specific) or asset(generic)
                        if value:
                            backgrounds[state] = value
                        special_prefix = f'GFX_technology_{folder}_special_project_' + ('small_' if small else '')
                        special = asset(special_prefix + state + '_item_bg')
                        if special:
                            specials[state] = special
                    declared = _point(node, 'size', ('width', 'height'))
                    normal = backgrounds.get('unavailable', {})
                    template = {'name': name, 'folder_id': folder, 'variant': 'equipment' if text else 'small',
                                'position': _point(node), 'declared_size': declared,
                                'size': {k: normal.get(k, declared[k]) for k in ('width', 'height')},
                                'icon': {**_point(icon), 'centered': _field(icon, 'centerposition') == 'yes'},
                                'states': backgrounds, 'special_project_states': specials, 'source': str(file)}
                    if text:
                        template['label'] = {**_point(text), 'max_width': _number(text, 'maxwidth'),
                                             'max_height': _number(text, 'maxheight'), 'font': _field(text, 'font')}
                    result['templates'][name] = template
                elif name.endswith('folder') or name.endswith('_folder'):
                    layers = list(layer_walk(node, parent_size={'width': 2522, 'height': 1276}))
                    if layers:
                        result['folders'][name] = {'layers': layers, 'source': str(file)}
        except (OSError, ValueError, UnicodeError, RecursionError) as exc:
            result['warnings'].append(f'Technology visuals not readable ({file.name}): {exc}')
    return result
