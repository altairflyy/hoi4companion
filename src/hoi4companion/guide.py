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
    from .backend_core import _request
    from .research_upgrades import guide_language
    def focus(value):
        fields={'name':'','cost':10,'prerequisites':[],'mutually_exclusive':[],'criteria':None,'has_conditions':False}
        return {key:getattr(value,key,default) for key,default in fields.items()}
    return _request(0,'source_enrich',guide=guide,vocabulary=guide_language(),
                    trees={key:{'focuses':{identifier:focus(value) for identifier,value in tree.focuses.items()}} for key,tree in trees.items()},
                    technologies=technologies,labels=labels,folders=folders,
                    continuous={key:focus(value) for key,value in (continuous_focuses or {}).items()})
