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


def apply_technology_layout(catalog,gui_files,labels):
    from .backend_core import _request
    value=_request(0,'technology_layout',catalog=catalog,files=list(map(str,gui_files)),labels=labels)
    catalog.clear();catalog.update(value)
