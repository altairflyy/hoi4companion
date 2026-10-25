"""Small local identity corrections, separate from guide sources and run progress."""
from copy import deepcopy
import json
import os
from pathlib import Path
import re
import tempfile
from .paths import data_directory

DIRECTORY=data_directory()/'guide-associations'


def _path(guide_id,directory):
    if not isinstance(guide_id,str) or not re.fullmatch(r'[A-Za-z0-9_-]{1,160}',guide_id):raise ValueError('ID guida non valido')
    return Path(directory if directory is not None else DIRECTORY)/(guide_id+'.json')


def read_associations(guide_id,directory=None):
    path=_path(guide_id,directory)
    if not path.exists():return {'schema_version':1,'guide_id':guide_id,'changes':[]}
    if path.stat().st_size>2_000_000:raise ValueError('Correzioni associazioni troppo grandi')
    result=json.loads(path.read_text(encoding='utf-8'))
    if (not isinstance(result,dict) or result.get('schema_version')!=1 or result.get('guide_id')!=guide_id
            or not isinstance(result.get('changes'),list) or len(result['changes'])>4096):raise ValueError('Correzioni associazioni non valide')
    from .contracts import validate_entity
    keys=set()
    for item in result['changes']:
        if not isinstance(item,dict) or not isinstance(item.get('key'),str) or not item['key'] or item['key'] in keys:raise ValueError('Correzione associazione duplicata o non valida')
        keys.add(item['key'])
        for field in ('original','entity'):
            validate_entity(item.get(field))
            if set(item[field])-{'kind','id','scope'}:raise ValueError('Campi associazione non validi')
        if item['original']['kind']!=item['entity']['kind']:raise ValueError('Tipo associazione cambiato')
    return result


def apply_associations(guide,directory=None):
    from .guide import normalise_guide
    result=normalise_guide(guide);changes=read_associations(result['id'],directory)['changes']
    rows={r['key']:r for lane in result['tracks'].values() for r in lane};warnings=[]
    for item in changes:
        row=rows.get(item['key'])
        if row is None or row['action'].get('entity')!=item['original']:
            warnings.append(item['key']+': definizione originale cambiata; correzione precedente non applicata.')
            continue
        row['action']['entity']=deepcopy(item['entity']);row['target']=item['entity']['id']
        row.pop('research_upgrade',None)
    if warnings:result['association_warnings']=warnings
    return normalise_guide(result)


def change_association(guide,key,expected,entity,catalog,directory=None):
    """Caller serializes writes. Never rewrite guide JSON or progress records."""
    from .guide import normalise_guide
    from .contracts import validate_entity
    from .focus_resolution import resolve_focus_routes
    if not isinstance(key,str) or not key:raise ValueError('Passaggio non valido')
    source=normalise_guide(guide);current=apply_associations(source,directory)
    rows={r['key']:r for lane in current['tracks'].values() for r in lane}
    originals={r['key']:r for lane in source['tracks'].values() for r in lane}
    row=rows.get(key)
    if not row or row['action']['kind'] not in ('select_focus','research','special_project'):raise ValueError('Passaggio senza associazione modificabile')
    old=row['action']['entity'];original=originals[key]['action']['entity']
    if expected!=old:raise ValueError('Associazione cambiata: ricarica prima di salvare')
    replacement=deepcopy(original if entity is None else entity)
    validate_entity(replacement)
    if set(replacement)-{'kind','id','scope'} or replacement['kind']!=old['kind']:raise ValueError('Tipo associazione incompatibile')
    if entity is not None:
        scope=source['applies_to'].get('tree_id') or old.get('scope')
        if old['kind']=='focus' and scope and replacement.get('scope')!=scope:raise ValueError('Albero focus incompatibile con la guida')
        country=source['applies_to'].get('country')
        matches=[item for item in catalog if item['kind']==replacement['kind'] and item['id']==replacement['id']
                 and (item['kind']!='focus' or item.get('scope')==replacement.get('scope'))
                 and (not country or not item.get('countries') or country in item['countries'])]
        if len(matches)!=1:raise ValueError('ID non presente o ambiguo nel catalogo compatibile')
    row['action']['entity']=replacement;row['target']=replacement['id'];row.pop('research_upgrade',None)
    current=normalise_guide(current)
    if old['kind']=='focus' and entity is not None:
        _,_,before_errors=resolve_focus_routes(deepcopy(apply_associations(source,directory)),catalog)
        _,_,after_errors=resolve_focus_routes(deepcopy(current),catalog)
        if set(after_errors)-set(before_errors):raise ValueError('; '.join(after_errors))
    data=read_associations(source['id'],directory)
    data['changes']=[item for item in data['changes'] if item['key']!=key]
    if replacement!=original:data['changes'].append({'key':key,'original':deepcopy(original),'entity':replacement})
    path=_path(source['id'],directory);path.parent.mkdir(parents=True,exist_ok=True)
    temp=None
    try:
        with tempfile.NamedTemporaryFile(mode='w',encoding='utf-8',dir=path.parent,suffix='.tmp',delete=False) as stream:
            temp=Path(stream.name);json.dump(data,stream,ensure_ascii=False,indent=2);stream.write('\n');stream.flush();os.fsync(stream.fileno())
        os.replace(temp,path)
    finally:
        if temp is not None:temp.unlink(missing_ok=True)
    return apply_associations(source,directory)
