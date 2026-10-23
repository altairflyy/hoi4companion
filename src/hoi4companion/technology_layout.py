"""Read-only reconstruction of the technology GUI's declared tabs and grids.

GUI alignments and item-template offsets are reconstructed from installed files
and corroborated against the user's game screenshots. No game script executes.
"""
from __future__ import annotations

import math
from .clausewitz import Node
from .jomini_parser import parse_file
from .tooltips import localise


def _numeric(node, key):
    if not isinstance(node, Node):
        return None
    try:
        number = float(node.scalar(key))
        return number if math.isfinite(number) else None
    except (ValueError, TypeError):
        return None


def _metric(raw, reference):
    try:
        value = float(raw.rstrip('%'))
        value = value * reference / 100 if raw.endswith('%') else value
        return value if math.isfinite(value) else None
    except (ValueError, TypeError, AttributeError):
        return None


def _walk(node, x=0.0, y=0.0, width=2522.0, height=1276.0):
    """Folder-local GUI coordinates, including parent-relative percentages."""
    for kind, child in node.entries:
        if not isinstance(child, Node):
            continue
        pos = child.first('position')
        px = _metric(pos.scalar('x'), width) if isinstance(pos, Node) else None
        py = _metric(pos.scalar('y'), height) if isinstance(pos, Node) else None
        # Shipped NSB GUI has the literal typo "275k" on this heading. The
        # supplied game screenshot places PESANTI at parent x + 275, rather
        # than dropping the title. Keep this evidence-bound repair narrow.
        if isinstance(pos, Node) and child.scalar('name') == 'armour_subtitle_heavy' and pos.scalar('x') == '275k':
            px = 275.0
        if isinstance(pos, Node) and ((pos.scalar('x') and px is None) or (pos.scalar('y') and py is None)):
            continue
        cx, cy = x + (px or 0), y + (py or 0)
        size = child.first('size', Node())
        cw, ch = _metric(size.scalar('width'), width), _metric(size.scalar('height'), height)
        minimum = size.first('min', Node())
        cw = max(cw or 0, _metric(minimum.scalar('width'), width) or 0) or width
        ch = max(ch or 0, _metric(minimum.scalar('height'), height) or 0) or height
        orientation = child.scalar('Orientation', child.scalar('orientation')).upper()
        if orientation in ('CENTER_UP', 'CENTER_DOWN', 'CENTER'):
            cx += width / 2
        if child.scalar('origo').upper() in ('CENTER_UP', 'CENTER_DOWN', 'CENTER'):
            cx -= cw / 2
        if orientation in ('UPPER_RIGHT', 'LOWER_RIGHT'):
            cx += width
        if orientation in ('LOWER_LEFT', 'LOWER_RIGHT'):
            cy += height
        yield kind.lower(), child, cx, cy
        if kind.lower() in ('containerwindowtype', 'guitypes'):
            yield from _walk(child, cx, cy, cw, ch)


def _technology_metadata(technologies, warnings):
    """Retain layout-specific fields the public research catalog doesn't need."""
    metadata = {}
    for path in dict.fromkeys(t.get('source') for t in technologies.values() if t.get('source')):
        try:
            from pathlib import Path
            root = parse_file(Path(path))
            for block in root.blocks('technologies'):
                for key, node in block.entries:
                    if not isinstance(node, Node):
                        continue
                    paths = [p.scalar('leads_to_tech') for p in node.blocks('path') if p.scalar('ignore_for_layout') != 'yes']
                    ignored = [p.scalar('leads_to_tech') for p in node.blocks('path') if p.scalar('ignore_for_layout') == 'yes']
                    equipment = (bool(node.blocks('enable_equipments')) or node.scalar('show_equipment_icon') == 'yes') and node.scalar('force_use_small_tech_layout') != 'yes'
                    metadata[key] = {'paths': paths, 'ignored': ignored, 'equipment': equipment,
                                     'gated': bool(node.blocks('allow_branch'))}
        except (OSError, ValueError, UnicodeError, RecursionError) as exc:
            warnings.append(f'Metadati GUI ricerca non leggibili: {exc}')
    return metadata


def apply_technology_layout(catalog, gui_files, labels):
    folders, technologies = catalog['folders'], catalog['technologies']
    definitions, tabs, templates = {}, {}, {}
    metadata = _technology_metadata(technologies, catalog['warnings'])
    for key, tech in technologies.items():
        tech['layout_ignored_paths'] = metadata.get(key, {}).get('ignored', [])
    for file in gui_files:
        try:
            root = parse_file(file)
            for kind, window, _, _ in _walk(root):
                if kind != 'containerwindowtype':
                    continue
                name = window.scalar('name')
                if name.startswith('techtree_') and name.endswith('_item'):
                    pos, size = window.first('position', Node()), window.first('size', Node())
                    has_name = any(isinstance(child, Node) and child.scalar('name') == 'Name' for _, child in window.entries)
                    templates[name] = {'offset_x': _numeric(pos, 'x') or 0, 'offset_y': _numeric(pos, 'y') or 0,
                                       'declared_width': _numeric(size, 'width'), 'declared_height': _numeric(size, 'height'),
                                       'has_name': has_name}
                if name == 'folder_tabs':
                    for child_kind, button, x, _ in _walk(window):
                        if child_kind == 'buttontype' and button.scalar('name').endswith('_tab'):
                            folder = button.scalar('name')[:-4]
                            tabs[folder] = {'order': x, 'sprite_name': button.scalar('quadTextureSprite'), 'source': str(file)}
                elif name in folders:
                    grids, captions = [], []
                    margin = window.first('margin', Node())
                    content_width = 2560 - (_numeric(margin, 'left') or 0) - (_numeric(margin, 'right') or 0)
                    content_height = 1360 - 47 - (_numeric(margin, 'top') or 0) - (_numeric(margin, 'bottom') or 0)
                    for child_kind, child, x, y in _walk(window, width=content_width, height=content_height):
                        child_name = child.scalar('name')
                        if child_kind == 'gridboxtype' and child_name.endswith('_tree'):
                            slot = child.first('slotsize')
                            sw, sh = _numeric(slot, 'width'), _numeric(slot, 'height')
                            direction = child.scalar('format').upper()
                            if sw and sh and sw > 0 and sh > 0 and direction in ('LEFT', 'UP', 'UPPER_LEFT'):
                                size = child.first('size', Node())
                                grids.append({'id': child_name, 'root': child_name[:-5], 'x': x, 'y': y,
                                              'slot_width': sw, 'slot_height': sh, 'direction': direction,
                                              'width': _metric(size.scalar('width'), content_width) or 0,
                                              'height': _metric(size.scalar('height'), content_height) or 0})
                        elif child_kind == 'instanttextboxtype' and ('subtitle' in child_name or '_year' in child_name):
                            text = child.scalar('text')
                            if text:
                                captions.append({'text': localise(labels.get(text, text), labels), 'x': x, 'y': y,
                                                 'width': _numeric(child, 'maxWidth') or 200,
                                                 'height': _numeric(child, 'maxHeight') or 32,
                                                 'align': child.scalar('format', 'left'), 'font': child.scalar('font'),
                                                 'kind': 'year' if '_year' in child_name else 'title'})
                    definitions[name] = {'source': str(file), 'grids': grids, 'captions': captions,
                                         'width': content_width, 'height': content_height}
        except (OSError, ValueError, UnicodeError, RecursionError) as exc:
            catalog['warnings'].append(f'Disposizione ricerca non leggibile ({file.name}): {exc}')
    for folder_id, folder in folders.items():
        folder['tab'] = tabs.get(folder_id, {'order': 10000, 'sprite_name': ''})
        definition = definitions.get(folder_id)
        folder['sections'] = []
        if not definition:
            continue
        ids = set(folder['technologies'])
        roots = {g['root'] for g in definition['grids'] if g['root'] in ids}
        assignments, hidden = {}, {}
        for grid in definition['grids']:
            if grid['root'] in ids or grid['root'] not in metadata:
                continue
            pending, visited = [grid['root']], set()
            while pending:
                key = pending.pop()
                if key in visited:
                    continue
                visited.add(key)
                if key in ids:
                    hidden[key] = f"Radice {grid['root']} esclusa dalle condizioni DLC; il discendente rimane nei dati locali."
                pending.extend(metadata.get(key, {}).get('paths', []))
        for grid in definition['grids']:
            root = grid['root']
            if root not in roots:
                continue
            pending, visited = [root], set()
            while pending:
                key = pending.pop()
                if key in visited or key not in ids or (key in roots and key != root):
                    continue
                visited.add(key)
                assignments.setdefault(key, []).append(grid)
                pending.extend(metadata.get(key, {}).get('paths', [p['target'] for p in technologies[key]['paths']]))
        mapped = 0
        section_counts = {}
        for key in ids:
            grids = assignments.get(key, [])
            membership = next(m for m in technologies[key]['folders'] if m['id'] == folder_id)
            if key in hidden:
                membership['layout_hidden_reason'] = hidden[key]
                technologies[key]['layout_hidden_reason'] = hidden[key]
            if len(grids) != 1 or membership['x'] is None or membership['y'] is None:
                continue
            grid = grids[0]
            # GUI alignment positions slots; the item template then offsets its visible rectangle.
            x, y = membership['x'], membership['y']
            sx, sy = (y, x) if grid['direction'] == 'LEFT' else (x, y)
            ax, ay = grid['x'] + sx * grid['slot_width'], grid['y'] + sy * grid['slot_height']
            if grid['direction'] == 'UP':
                ax += grid['width'] / 2 - grid['slot_width'] / 2 if grid['width'] else 0
            elif grid['direction'] == 'LEFT':
                ay += grid['height'] / 2 - grid['slot_height'] / 2
            equipment = metadata.get(key, {}).get('equipment', False)
            template_name = f"techtree_{folder_id}{'' if equipment else '_small'}_item"
            if template_name not in templates:
                template_name = f'techtree_{folder_id}_item'
            template = templates.get(template_name, {'offset_x': 0, 'offset_y': 0, 'has_name': False})
            named = equipment and template['has_name']
            if named:
                width, height = (332, 69) if folder_id in ('naval_folder', 'mtgnavalfolder', 'mtgnavalsupportfolder') else (183, 84)
            else:
                width, height = 72, 72
            left, top = ax + template['offset_x'], ay + template['offset_y']
            membership['layout'] = {'x': left, 'y': top, 'width': width, 'height': height,
                                    'center_x': left + width / 2, 'center_y': top + height / 2,
                                    'anchor_x': ax, 'anchor_y': ay, 'template': template_name,
                                    'template_declared_width': template.get('declared_width'),
                                    'template_declared_height': template.get('declared_height'),
                                    'variant': 'equipment' if named else 'small',
                                    'direction': grid['direction'], 'tree_id': grid['id'],
                                    'section_id': grid['root'], 'basis': 'gui_alignment_item_template_and_path'}
            section_counts[grid['root']] = section_counts.get(grid['root'], 0) + 1
            mapped += 1
        for grid in definition['grids']:
            if grid['root'] in section_counts:
                folder['sections'].append({'id': grid['root'], 'name': technologies[grid['root']]['name'],
                                           'count': section_counts[grid['root']], 'direction': grid['direction']})
        folder['layout'] = {'basis': 'gui_alignment_and_item_templates', 'source': definition['source'],
                            'mapped': mapped, 'total': len(ids), 'captions': definition['captions'],
                            'width': definition['width'], 'height': definition['height'], 'reference_viewport_width': 2560,
                            'hidden': [{'id': key, 'reason': reason} for key, reason in sorted(hidden.items())],
                            'limitations': 'Ricostruzione delle griglie GUI su viewport di riferimento 2560 px; controllata sui landmark degli screenshot forniti.'}
