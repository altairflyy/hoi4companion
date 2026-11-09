"""Stable campaign bindings for research steps authored as relative upgrades."""
from copy import deepcopy
import re


def adapt_legacy(guide, by_id, cache=None):
    """Adapt documented legacy UPs in memory; never rewrite their guide file."""
    from .research_upgrades import relative_chain, chains_for_matches
    guide = deepcopy(guide)
    cache = cache if cache is not None else {}
    for rows in guide.get('tracks', {}).values():
        for step in rows:
            previous = step.get('research_upgrade')
            if step.get('relative_research') or not isinstance(previous,dict):
                continue
            query = previous.get('query')
            match = re.fullmatch(r'\s*(.+?)\s+UP\s*(?:\([^)]*\))?\s*', query, re.I) if isinstance(query,str) else None
            if step['action']['kind'] != 'research' or not match:
                continue
            anchor = previous.get('from_id')
            key = (query, anchor)
            if key not in cache:
                chain = relative_chain(match[1],by_id)
                if not chain and isinstance(anchor,str) and anchor in by_id:
                    chain = chains_for_matches([anchor],by_id)
                cache[key] = chain
            chain = cache[key]
            if chain:
                step['relative_research'] = {'query':query,'chain':list(chain)}
                step.pop('relative_research_unresolved',None)
            else:
                step['relative_research_unresolved'] = True
                guide.setdefault('association_warnings',[]).append(
                    step['key']+': ramo UP non identificato nel catalogo corrente; la vecchia associazione non viene usata.')
    return guide


def validate_relative(step):
    if 'relative_research_unresolved' in step and (step['action']['kind'] != 'research' or step['relative_research_unresolved'] is not True):
        raise ValueError('Stato ricerca relativa non valido')
    relative = step.get('relative_research')
    if relative is None:
        return
    if (step['action']['kind'] != 'research' or not isinstance(relative, dict)
            or set(relative) != {'query', 'chain'} or not isinstance(relative['query'], str)
            or not 0 < len(relative['query']) <= 512 or not isinstance(relative['chain'], list)
            or not 1 <= len(relative['chain']) <= 128
            or any(not isinstance(x, str) or not 0 < len(x) <= 512 for x in relative['chain'])
            or len(set(relative['chain'])) != len(relative['chain'])):
        raise ValueError('Ramo ricerca relativa non valido')


def project(guide, bindings):
    from .backend_core import _request
    return _request(0,'guide_research_projection',guide=guide,bindings=bindings)


def progression(guide, state):
    """Qualify progression separately from choosing/fixing a campaign target."""
    fact = state.get('facts', {}).get('research.completed', {})
    if (not state.get('usable') or fact.get('status') != 'observed' or not fact.get('source')
            or not isinstance(fact.get('value'), list)):
        return {}
    completed = set(fact['value'])
    result = {}
    for rows in guide['tracks'].values():
        previous = []
        for step in rows:
            if step['action']['kind'] != 'research':
                continue
            relative = step.get('relative_research')
            if relative:
                chain = relative['chain']; target = step['action']['entity']['id']
                if target in chain:
                    if any(x not in completed for x in chain[:chain.index(target)]):
                        result[step['key']] = 'research_previous_level_incomplete'
                elif all(x in completed for x in chain):
                    result[step['key']] = 'research_line_complete'
                elif any(p['action']['entity']['id'] in chain
                         and p['action']['entity']['id'] not in completed for p in previous):
                    result[step['key']] = 'research_previous_level_incomplete'
            previous.append(step)
    return result
