"""Curated video walkthrough; never execute source text or game conditions."""
from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path
from urllib.parse import urlencode

from .tooltips import localise


def load_guide():
    return json.loads((Path(__file__).parent / 'guides' / 'poland-wojtek-FBPdhlwqMwM.json').read_text(encoding='utf-8'))


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
    allowed = {s['key'] for steps in guide['tracks'].values() for s in steps if s['kind'] in ('gate','project')}
    allowed.update(s[field] for steps in guide['tracks'].values() for s in steps for field in ('gate','available_gate') if field in s)
    allowed.update(s['key'] for s in guide.get('milestones',[]))
    if not isinstance(update['key'],str) or update['key'] not in allowed:
        raise ValueError('Questa voce non è una conferma manuale della guida')
    result = dict(current)
    if update['checked']:
        result[update['key']] = True
    else:
        result.pop(update['key'],None)
    return result


def enrich_guide(guide, trees, technologies, labels, folders):
    result = deepcopy(guide)
    warnings = []
    tree = trees.get(guide['tree_id'])
    focuses = tree.focuses if tree else {}
    for track in result['tracks'].values():
        for step in track:
            kind, target = step['kind'], step.get('target')
            if kind == 'focus':
                focus = focuses.get(target)
                step['catalog_match'] = focus is not None
                if focus:
                    step.update(name=focus.name, estimated_days=round(focus.cost * 7), prerequisites=focus.prerequisites,
                                href='/?' + urlencode({'tree':guide['tree_id'],'focus':target}))
                else:
                    warnings.append(f'Focus non trovato nel catalogo compatibile: {target}')
            elif kind == 'technology':
                tech = technologies.get(target)
                step['catalog_match'] = tech is not None
                if tech:
                    equipment = next((u['id'] for u in tech.get('unlocks',[]) if u['kind']=='Equipaggiamento'), '')
                    keys = [guide['country']+'_'+target, guide['country']+'_'+equipment, target, equipment]
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
    result['catalog_warnings']=warnings
    return result
