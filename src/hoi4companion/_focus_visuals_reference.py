"""Read focus GUI geometry; no gameplay conditions or process access.

Use the same read-only parser and texture-header reader as technology visuals.
Definitions follow installation/mod file precedence. Missing metrics stay absent.
"""
from pathlib import Path
import re

from .jomini_parser import parse_file
from .technology_visuals import _field, _number, _point, _walk, _dimensions

POSITIONS = ('focus_spacing', 'national_focus_center', 'link_spacing', 'link_offsets',
             'link_begin', 'link_end', 'exclusive_offset', 'exclusive_offset_left',
             'exclusive_positioning')
TEMPLATES = ('national_focus_item', 'national_focus_link', 'national_focus_exclusive_item')


def load_focus_visuals(gui_files, sprite_library=None):
    result = {'schema_version': 1, 'basis': 'installed_gui', 'status': 'unavailable',
              'positions': {}, 'templates': {}, 'states': {}, 'warnings': []}
    for file in map(Path, gui_files):
        try:
            # Custom mod filenames can override these definitions too. Avoid
            # parsing unrelated GUI documents or measuring all game textures.
            if not re.search(r'\b(?:' + '|'.join((*POSITIONS, *TEMPLATES)) + r')\b', file.read_text(encoding='utf-8-sig', errors='replace')):
                continue
            for kind, node in _walk(parse_file(file)):
                name = _field(node, 'name')
                if name in POSITIONS and kind == 'positiontype':
                    point = _point(node)
                    if all(v is not None and abs(v) <= 65536 for v in point.values()):
                        result['positions'][name] = {**point, 'source': str(file)}
                    else:
                        result['positions'].pop(name, None)
                elif name in TEMPLATES and kind == 'containerwindowtype':
                    size = _point(node, 'size', ('width', 'height'))
                    children = {}
                    for child_kind, child in node.entries:
                        if child_kind.casefold() not in ('icontype', 'buttontype', 'instanttextboxtype'):
                            continue
                        child_name = _field(child, 'name')
                        if not isinstance(child_name, str) or not child_name:
                            continue
                        children[child_name] = {
                            'position': _point(child),
                            'sprite_name': _field(child, 'spritetype') or _field(child, 'quadtexturesprite'),
                            'frame': _number(child, 'frame', 1),
                            'center': _field(child, 'centerposition') == 'yes',
                            'orientation': str(_field(child, 'orientation', 'UPPER_LEFT')).upper(),
                            'scale': _number(child, 'scale', 1),
                            'width': _number(child, 'maxwidth'), 'height': _number(child, 'maxheight'),
                            'font': _field(child, 'font'), 'align': _field(child, 'format', 'center')}
                    result['templates'][name] = {'size': size, 'position': _point(node),
                                                  'children': children, 'source': str(file)}
        except (OSError, ValueError, RecursionError) as exc:
            result['warnings'].append(f'Focus GUI non leggibile ({file.name}): {exc}')
    spacing = result['positions'].get('focus_spacing', {})
    size = result['templates'].get('national_focus_item', {}).get('size', {})
    if all(isinstance(v, (int, float)) and 0 < v <= 4096 for v in
           (spacing.get('x'), spacing.get('y'), size.get('width'), size.get('height'))):
        result['status'] = 'observed'
    if sprite_library is not None:
        measured = {}
        def asset(name, frame=1):
            if not isinstance(frame, (int, float)) or not float(frame).is_integer():
                return None
            value = sprite_library.resolve(name, int(frame)) if name else None
            if not value:
                return None
            definition = sprite_library.sprites.get(name)
            if name not in measured:
                path = sprite_library._texture(definition.texture) if definition else None
                measured[name] = _dimensions(path, definition.frames) if path else None
            return {**value, **(measured[name] or {})}
        states = {'locked': 'unavailable', 'unknown': 'unavailable', 'verify': 'unavailable',
                  'excluded': 'unavailable', 'ready': 'can_start', 'active': 'current', 'completed': 'completed'}
        result['states'] = {key: value for key, suffix in states.items()
                            if (value := asset('GFX_focus_' + suffix))}
        link_names = ('up_down', 'left_right', 'down_right', 'down_left', 'up_right', 'up_left',
                      'down_left_right', 'up_left_right', 'up_down_left', 'up_down_right', 'up_down_left_right')
        # Shipped strips: green solid/dashed then blue solid/dashed. The GUI
        # placeholder frame=1 is green; it is not evidence of completion.
        result['link_states'] = {state: {suffix: value for suffix in link_names
            if (value := asset('GFX_focus_link_' + suffix, frame))}
            for state, frame in (('neutral', 3), ('alternative', 4), ('completed', 1), ('completed_alternative', 2))}
        result['links'] = result['link_states']['neutral']
        for template in result['templates'].values():
            for child in template['children'].values():
                child['sprite'] = asset(child['sprite_name'], child['frame'])
    return result
