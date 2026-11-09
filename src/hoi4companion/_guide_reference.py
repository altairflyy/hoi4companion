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
from .paths import data_directory


GUIDES = Path(__file__).parent / 'guides'
LOCAL_GUIDES = data_directory() / 'guides'


def normalise_guide(guide):
    """Normalize and validate Guide Schema v1 in the universal Rust backend."""
    from .backend_core import _request
    return _request(0,'guide_normalize',guide=guide)


@lru_cache(maxsize=64)
def _load_file(path, mtime_ns, size):
    return normalise_guide(json.loads(Path(path).read_text(encoding='utf-8')))


def _catalog(directory=None):
    catalog = {}
    paths = sorted(Path(directory).glob('*.json')) if directory is not None else [
        *sorted(GUIDES.glob('*.json')), *sorted(LOCAL_GUIDES.glob('*.json'))]
    for path in paths:
        stat = path.stat()
        guide = _load_file(str(path.resolve()), stat.st_mtime_ns, stat.st_size)
        key = guide.get('id')
        if not isinstance(key, str) or not re.fullmatch(r'[A-Za-z0-9_-]{1,160}', key) or key in catalog:
            raise ValueError('Identità guida assente, duplicata o non valida')
        catalog[key] = guide
    return catalog


def list_guides(directory=None):
    return [{'id': g['id'], 'title': g['title'], 'schema_version': VERSION, 'applies_to': deepcopy(g['applies_to'])}
            for g in _catalog(directory).values()]


def _default(catalog):
    defaults = [key for key, guide in catalog.items() if guide.get('default') is True]
    if len(defaults) > 1:
        raise ValueError('Più guide predefinite dichiarate')
    return defaults[0] if defaults else next(iter(catalog), None)


def default_guide_id(directory=None):
    return _default(_catalog(directory))


def load_guide(guide_id=None, *, directory=None, associations=True):
    catalog = _catalog(directory)
    if guide_id is None:
        guide_id = _default(catalog)
    if not isinstance(guide_id, str) or guide_id not in catalog:
        raise ValueError('Guida non trovata')
    result=deepcopy(catalog[guide_id])
    if associations:
        from .guide_associations import apply_associations
        result=apply_associations(result)
    return result


def manual_keys(guide):
    allowed = {s['key'] for rows in guide['tracks'].values() for s in rows
               if s['kind'] in ('gate', 'project') and s.get('tracking') != 'reminder'}
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


def enrich_guide(guide, trees, technologies, labels, folders, *,continuous_focuses=None):
    result = normalise_guide(guide)
    warnings = list(result.get('association_warnings',[]))
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
                continuous = (continuous_focuses or {}).get(target)
                focus = continuous or (tree.focuses.get(target) if tree else None)
                step['catalog_match'] = focus is not None
                if focus:
                    if continuous:
                        step['action'].setdefault('params',{})['focus_mode']='continuous'
                        step.update(name=focus.name,continuous=True,has_conditions=focus.has_conditions,prerequisites=[])
                        step.pop('estimated_days',None)
                        step.pop('href',None)
                        continue
                    step['action']['entity'].setdefault('scope', tree_id)
                    step.update(name=focus.name, estimated_days=round(focus.cost * 7), prerequisites=focus.prerequisites,
                                href='/?' + urlencode({'tree':tree_id,'focus':target}))
                    if getattr(focus,'criteria',None):
                        # Only irreversible graph exclusions are projected;
                        # dynamic availability is evaluated from live facts.
                        mutex=set(focus.mutually_exclusive)|{fid for fid,other in tree.focuses.items() if target in other.mutually_exclusive}
                        excluded=set(mutex)
                        changed=True
                        while changed:
                            changed=False
                            for fid,other in tree.focuses.items():
                                if fid not in excluded and any(g and all(p in excluded for p in g) for g in other.prerequisites):
                                    excluded.add(fid);changed=True
                        step['focus_criteria']={**deepcopy(focus.criteria),'prerequisites':deepcopy(focus.prerequisites),
                                                'mutex':sorted(mutex),'conflicts':sorted(excluded)}
                else:
                    warnings.append(f'Focus non trovato nel catalogo compatibile: {target}')
            elif kind == 'technology':
                if step.get('research_choice'):
                    step['choice_entities']={}
                    for identity in {x for chain in step['research_choice']['branches'].values() for x in chain}:
                        option=technologies.get(identity)
                        if not option:continue
                        folder=next((f['id'] for f in option.get('folders',[]) if f['id'] in folders and f.get('layout')),None)
                        step['choice_entities'][identity]={'id':identity,'name':localise(labels.get(identity,option['name']),labels),
                            'folder':folder,'folder_name':folders.get(folder,{}).get('name'),
                            'href':'/technologies?'+urlencode({'folder':folder,'tech':identity}) if folder else None}
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
                elif not (((step.get('relative_research') or step.get('relative_research_unresolved')) and target.startswith('unbound-up:')) or step.get('research_choice') and target.startswith('unbound-choice:')):
                    warnings.append(f'Tecnologia non trovata nel catalogo compatibile: {target}')
            elif kind in ('gate','project'):
                step['verification']='automatic' if step.get('completion') else 'start_only' if step.get('started_when') else 'manual'
                if target in labels:
                    step['name']=localise(labels[target],labels)
            step.setdefault('name',step.get('title',target or kind))
            if step['action']['kind'] not in ('select_focus','research','free'):
                from .informal_guides import explanatory_terms
                params=step['action'].get('params',{})
                instructions=params.get('instructions',[])
                step['operational_glossary']=explanatory_terms('\n'.join(s for s in instructions if isinstance(s,str)) if isinstance(instructions,list) else '',params.get('category'))
            if step['action'].get('entity'):
                step['entity'] = entity(step['action']['entity'], step['name'], {'catalog_match': step.get('catalog_match')})
    result['catalog_warnings']=warnings
    return result
