"""Guide catalog, legacy migration and declarative validation; no acquisition."""
from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path
from functools import lru_cache
import re
from urllib.parse import urlencode

from .tooltips import localise
from .contracts import VERSION, entity, is_json_number, validate_action, validate_condition
from .guide_schema import validate_guide_extensions


GUIDES = Path(__file__).parent / 'guides'


def normalise_guide(guide):
    """Migrate legacy row conventions into country-neutral declarative policies."""
    if not isinstance(guide, dict):
        raise ValueError('Guida non valida')
    result = deepcopy(guide)
    if any(not is_json_number(result.get(field, default)) or result.get(field, default) != default
           for field, default in (('schema_version', VERSION), ('schema', 1))):
        raise ValueError('Versione contratto guida non supportata')
    result['schema_version'] = VERSION
    result.setdefault('applies_to', {k: result[k] for k in ('country', 'tree_id') if result.get(k)})
    if not isinstance(result['applies_to'], dict) or any(k not in ('country', 'tree_id') or not isinstance(v, str) or not v for k, v in result['applies_to'].items()):
        raise ValueError('Ambito guida non valido')
    for field in ('country', 'tree_id'):
        if result['applies_to'].get(field):
            if result.get(field) and result[field] != result['applies_to'][field]:
                raise ValueError('Ambito guida in conflitto con i campi legacy')
            result.setdefault(field, result['applies_to'][field])
    if not isinstance(result.get('tracks'), dict) or len(result['tracks']) > 32:
        raise ValueError('Tracce guida non valide')
    keys = set()
    actions = {'focus': 'select_focus', 'technology': 'research', 'project': 'special_project', 'gate': 'manual', 'free': 'free'}
    kinds = {v: k for k, v in actions.items()}
    for track, steps in result['tracks'].items():
        if not isinstance(track, str) or not isinstance(steps, list) or len(steps) > 4096:
            raise ValueError('Traccia guida non valida')
        for index, step in enumerate(steps):
            if not isinstance(step, dict):
                raise ValueError('Passaggio guida non valido')
            step.setdefault('key', f'{track}-{index + 1}')
            key = step['key']
            if not isinstance(key, str) or not key or key in keys:
                raise ValueError('Chiave passaggio assente o duplicata')
            keys.add(key)
            if 'action' not in step:
                kind = step.get('kind')
                if kind not in actions:
                    raise ValueError('Tipo passaggio non supportato')
                step['action'] = {'kind': actions[kind]}
                if kind in ('focus', 'technology', 'project'):
                    step['action']['entity'] = {'kind': kind, 'id': step.get('target', '')}
            validate_action(step['action'])
            ref = step['action'].get('entity')
            if ref and ref['kind'] == 'focus' and result['applies_to'].get('tree_id'):
                if ref.get('scope', result['applies_to']['tree_id']) != result['applies_to']['tree_id']:
                    raise ValueError('Ambito focus in conflitto con la guida')
                ref.setdefault('scope', result['applies_to']['tree_id'])
            step['kind'] = kinds[step['action']['kind']]
            if step['action'].get('entity'):
                step['target'] = step['action']['entity']['id']
            if step.get('gate'):
                step.setdefault('when', {'op': 'check', 'key': step['gate']})
            if step.get('available_gate') and step.get('priority_after'):
                step.setdefault('priority', 100)
                step.setdefault('priority_when', {'op': 'all', 'args': [
                    {'op': 'fact', 'key': 'focus.completed', 'test': 'contains', 'value': step['priority_after']},
                    {'op': 'check', 'key': step['available_gate']}]})
    steps = [s for rows in result['tracks'].values() for s in rows]
    for step in steps:
        if step.get('optional_before') and 'skip_when' not in step:
            policies = [s['priority_when'] for s in steps if s.get('target') == step['optional_before'] and s.get('priority_when')]
            conditions = [{'op': 'fact', 'key': 'focus.completed', 'test': 'contains', 'value': step['optional_before']}, *policies]
            step['skip_when'] = {'op': 'any', 'args': conditions}
        for field in ('when', 'priority_when', 'skip_when'):
            if field in step:
                validate_condition(step[field])
        if 'priority' in step:
            if not is_json_number(step['priority']) or step['priority'] != int(step['priority']) or not -100000 <= step['priority'] <= 100000:
                raise ValueError('Priorità guida non valida')
            step['priority'] = int(step['priority'])
    result.setdefault('milestones', [])
    result.setdefault('assumptions', [])
    result.setdefault('title', result.get('id', 'Guida'))
    result.setdefault('subtitle', '')
    validate_guide_extensions(result)
    return result


@lru_cache(maxsize=64)
def _load_file(path, mtime_ns, size):
    return normalise_guide(json.loads(Path(path).read_text(encoding='utf-8')))


def _catalog(directory):
    catalog = {}
    for path in sorted(Path(directory).glob('*.json')):
        stat = path.stat()
        guide = _load_file(str(path.resolve()), stat.st_mtime_ns, stat.st_size)
        key = guide.get('id')
        if not isinstance(key, str) or not re.fullmatch(r'[A-Za-z0-9_-]{1,160}', key) or key in catalog:
            raise ValueError('Identità guida assente, duplicata o non valida')
        catalog[key] = guide
    return catalog


def list_guides(directory=GUIDES):
    return [{'id': g['id'], 'title': g['title'], 'schema_version': VERSION, 'applies_to': deepcopy(g['applies_to'])}
            for g in _catalog(directory).values()]


def _default(catalog):
    defaults = [key for key, guide in catalog.items() if guide.get('default') is True]
    if len(defaults) > 1:
        raise ValueError('Più guide predefinite dichiarate')
    return defaults[0] if defaults else next(iter(catalog), None)


def default_guide_id(directory=GUIDES):
    return _default(_catalog(directory))


def load_guide(guide_id=None, *, directory=GUIDES):
    catalog = _catalog(directory)
    if guide_id is None:
        guide_id = _default(catalog)
    if not isinstance(guide_id, str) or guide_id not in catalog:
        raise ValueError('Guida non trovata')
    return deepcopy(catalog[guide_id])


def manual_keys(guide):
    allowed = {s['key'] for rows in guide['tracks'].values() for s in rows if s['kind'] in ('gate', 'project')}
    allowed.update(s['key'] for s in guide.get('milestones', []))
    def visit(node):
        if node.get('op') == 'check':
            allowed.add(node['key'])
        for child in node.get('args', []):
            visit(child)
        if node.get('arg'):
            visit(node['arg'])
    for rows in guide['tracks'].values():
        for step in rows:
            for field in ('when', 'priority_when', 'skip_when', 'completion', 'started_when'):
                if field in step:
                    visit(step[field])
            allowed.update(step[f] for f in ('gate', 'available_gate') if f in step)
    for phase in guide.get('phases', []):
        if 'when' in phase:
            visit(phase['when'])
    return allowed


def update_checks(guide, current, update):
    if update == {'reset': True}:
        return {}
    if isinstance(update,dict) and set(update) == {'checks'} and isinstance(update['checks'],dict):
        result = {}
        for key, checked in update['checks'].items():
            result = update_checks(guide,result,{'key':key,'checked':checked})
        return result
    if not isinstance(update,dict) or set(update) != {'key','checked'} or type(update['checked']) is not bool:
        raise ValueError('Conferma manuale non valida')
    allowed = manual_keys(guide)
    if not isinstance(update['key'],str) or update['key'] not in allowed:
        raise ValueError('Questa voce non è una conferma manuale della guida')
    result = dict(current)
    if update['checked']:
        result[update['key']] = True
    else:
        result.pop(update['key'],None)
    return result


def enrich_guide(guide, trees, technologies, labels, folders):
    result = normalise_guide(guide)
    warnings = []
    for track in result['tracks'].values():
        for step in track:
            kind, target = step['kind'], step.get('target')
            if kind == 'focus':
                tree_id = step['action']['entity'].get('scope') or result.get('tree_id')
                tree = trees.get(tree_id)
                if not tree_id:
                    matches = [(key, tree) for key, tree in trees.items() if target in tree.focuses]
                    if len(matches) == 1:
                        tree_id, tree = matches[0]
                focus = tree.focuses.get(target) if tree else None
                step['catalog_match'] = focus is not None
                if focus:
                    step['action']['entity'].setdefault('scope', tree_id)
                    step.update(name=focus.name, estimated_days=round(focus.cost * 7), prerequisites=focus.prerequisites,
                                href='/?' + urlencode({'tree':tree_id,'focus':target}))
                else:
                    warnings.append(f'Focus non trovato nel catalogo compatibile: {target}')
            elif kind == 'technology':
                tech = technologies.get(target)
                step['catalog_match'] = tech is not None
                if tech:
                    equipment = next((u['id'] for u in tech.get('unlocks',[]) if u['kind']=='Equipaggiamento'), '')
                    country = result.get('country', '')
                    keys = [country+'_'+target, country+'_'+equipment, target, equipment]
                    name = next((labels[k] for k in keys if k in labels),tech['name'])
                    folder = next((f['id'] for f in tech.get('folders',[]) if f['id'] in folders and f.get('layout')),None)
                    step.update(name=localise(name,labels), year=tech.get('start_year'), folder=folder,
                                folder_name=folders.get(folder,{}).get('name'), has_conditions=tech.get('has_conditions',False),
                                special_project_tech=tech.get('special_project',False))
                    if folder:
                        step['href']='/technologies?' + urlencode({'folder':folder,'tech':target})
                else:
                    warnings.append(f'Tecnologia non trovata nel catalogo compatibile: {target}')
            elif kind in ('gate','project'):
                step['verification']='manual'
                if target in labels:
                    step['name']=localise(labels[target],labels)
            step.setdefault('name',step.get('title',target or kind))
            if step['action'].get('entity'):
                step['entity'] = entity(step['action']['entity'], step['name'], {'catalog_match': step.get('catalog_match')})
    result['catalog_warnings']=warnings
    return result
