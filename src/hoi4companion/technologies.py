"""Catalogo ricerca dai file locali: nessuna simulazione della ricercabilità."""
from __future__ import annotations

import math
from pathlib import Path
from .clausewitz import Node, parse
from .eligibility import condition
from .tooltips import localise


def _normal(node):
    return Node([(k.upper() if k.upper() in ('AND', 'OR', 'NOT', 'NAND', 'NOR') else k,
                  _normal(v) if isinstance(v, Node) else v) for k, v in node.entries])


def _values(node):
    return [v for _, v in node.entries if isinstance(v, str)] if isinstance(node, Node) else []


def _number(raw, variables):
    seen = set()
    while raw.startswith('@') and raw not in seen:
        seen.add(raw)
        raw = variables.get(raw, '')
    try:
        value = float(raw)
        return value if math.isfinite(value) else None
    except (TypeError, ValueError):
        return None


def load_technologies(files, tag_files, labels, dlcs, scripts, country='AUS', gui_files=()):
    """Filter folder membership/allow_branch only; keep runtime allow as a caveat."""
    folders, technologies, warnings, excluded = {}, {}, [], []
    def check(block):
        return condition(_normal(block), country, dlcs, scripts) if dlcs else None
    for file in tag_files:
        try:
            root = parse(file.read_text(encoding='utf-8-sig'))
            for group in root.blocks('technology_folders'):
                for key, node in group.entries:
                    if not isinstance(node, Node) or node.scalar('doctrine') == 'yes':
                        continue
                    available = check(node.first('available')) if node.blocks('available') else True
                    if available is not True:
                        excluded.append({'id': key, 'reason': 'Condizioni DLC non corrispondenti' if available is False else 'Disponibilità non verificata'})
                        continue
                    folders[key] = {'id': key, 'name': localise(labels.get(key, key), labels),
                                    'ledger': node.scalar('ledger'), 'technologies': []}
        except (OSError, ValueError, UnicodeError, RecursionError) as exc:
            warnings.append(f'Categorie ricerca non leggibili ({file.name}): {exc}')
    for file in files:
        try:
            root = parse(file.read_text(encoding='utf-8-sig'))
            for group in root.blocks('technologies'):
                variables = {k: v for k, v in [*root.entries, *group.entries] if k.startswith('@') and isinstance(v, str)}
                for key, node in group.entries:
                    if not isinstance(node, Node) or key.startswith('@'):
                        continue
                    branch = check(node.first('allow_branch')) if node.blocks('allow_branch') else True
                    if branch is not True:
                        continue
                    memberships = []
                    for folder in node.blocks('folder'):
                        if folder.scalar('name') in folders:
                            pos = folder.first('position', Node())
                            memberships.append({'id': folder.scalar('name'),
                                                'x': _number(pos.scalar('x'), variables),
                                                'y': _number(pos.scalar('y'), variables)})
                    paths = [{'target': p.scalar('leads_to_tech'),
                              'coefficient': _number(p.scalar('research_cost_coeff'), variables)}
                             for p in node.blocks('path') if p.scalar('leads_to_tech')]
                    unlocks = []
                    for field, title in [('enable_equipments', 'Equipaggiamento'),
                                         ('enable_equipment_modules', 'Moduli'), ('enable_subunits', 'Unità')]:
                        for block in node.blocks(field):
                            for item in _values(block):
                                unlocks.append({'kind': title, 'id': item, 'name': localise(labels.get(item, item), labels)})
                    requirements = []
                    for block in node.blocks('dependencies'):
                        requirements += [{'id': k, 'value': v} for k, v in block.entries if k and isinstance(v, str)]
                    # Equipment-unlocking technologies often have no own localisation key.
                    equipment = next((u['id'] for u in unlocks if u['kind'] == 'Equipaggiamento'), '')
                    name_keys = [f'{country}_{key}', f'{country}_{equipment}', key, equipment]
                    name_key = next((k for k in name_keys if k in labels), key)
                    description_key = next((k + '_desc' for k in [*name_keys, name_key] if k + '_desc' in labels), '')
                    technologies[key] = {
                        'id': key, 'name': localise(labels.get(name_key, key), labels),
                        'description': localise(labels.get(description_key, ''), labels),
                        'source': str(file), 'research_cost': _number(node.scalar('research_cost'), variables),
                        'start_year': _number(node.scalar('start_year'), variables),
                        'folders': memberships, 'paths': paths, 'dependencies': requirements,
                        'exclusions': [v for k, b in node.entries if k.lower() == 'xor' for v in _values(b)],
                        'sub_technologies': [v for b in node.blocks('sub_technologies') for v in _values(b)],
                        'special_project': node.scalar('is_special_project_tech') == 'yes',
                        'has_conditions': bool(node.blocks('allow')), 'unlocks': unlocks,
                    }
        except (OSError, ValueError, UnicodeError, RecursionError) as exc:
            warnings.append(f'Tecnologie non leggibili ({file.name}): {exc}')
    for tech in technologies.values():
        for folder in tech['folders']:
            folders[folder['id']]['technologies'].append(tech['id'])
    folders = {k: v for k, v in folders.items() if v['technologies']}
    catalog = {'folders': folders, 'technologies': technologies, 'warnings': warnings,
               'excluded_folders': excluded}
    if gui_files:
        from .technology_layout import apply_technology_layout
        apply_technology_layout(catalog, gui_files, labels)
    return catalog
