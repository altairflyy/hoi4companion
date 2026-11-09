"""Generic province-level fortification goals; static topology is not ownership."""
from copy import deepcopy
import re
from .contracts import fact

BUILDINGS = {'bunker', 'coastal_bunker'}


def command(text, catalog):
    text = re.sub(r'^\s*BUILD\s*:\s*','',text,flags=re.I).strip()
    first, separator, remainder = text.partition(',')
    hit = re.fullmatch(r'(?:build\s+)?(\d+|max(?:imum)?)\s+(?:(land|coastal)\s+)?forts?\s+(?:in|on)\s+(.+)', first.strip(), re.I)
    if not hit:
        return None
    building = 'coastal_bunker' if (hit[2] or '').lower() == 'coastal' else 'bunker'
    entries = [row for row in catalog if row.get('kind') == 'building' and row.get('id') == building]
    if len(entries) != 1:
        return None
    level = entries[0].get('max_level') if hit[1].lower().startswith('max') else int(hit[1])
    if type(level) is not int or not 1 <= level <= 1000:
        return None
    params = {'category':'construction', 'building_id':building, 'quantity_mode':'target_total',
              'quantity':level, 'target_level':level}
    border = re.fullmatch(r'each\s+(?:tile|province)\s+(?:at|on|along)\s+([A-Z0-9]{3})\s+(?:border|frontier)',hit[3],re.I)
    if border:
        params['province_selector'] = {'kind':'border','country':border[1].upper()}
    elif re.fullmatch(r'(?:province\s+)?\d+',hit[3],re.I):
        params['province_id'] = int(re.search(r'\d+',hit[3])[0])
    else:
        # A named state does not identify a particular tile. The geographic
        # resolver can uniquely resolve a city, otherwise it keeps ambiguity.
        params['state_name'] = hit[3].strip()
    return {'params':params,'remainder':remainder.strip() if separator else ''}


def bind(step, catalog):
    p = step['action'].get('params',{})
    if p.get('category') != 'construction' or p.get('building_id') not in BUILDINGS:
        return False
    target = p.get('target_level',p.get('target_quantity'))
    if target is None and p.get('quantity_mode') == 'target_total':
        target = p.get('quantity')
    if type(target) is not int or not 1 <= target <= 1000:
        return False
    limits={row['max_level'] for row in catalog if row.get('kind')=='building' and row.get('id')==p['building_id']
            and type(row.get('max_level')) is int}
    if len(limits)==1 and target>next(iter(limits)):
        return False
    selector = p.get('province_selector')
    provinces = p.get('province_ids', [p['province_id']] if 'province_id' in p else None)
    border = (isinstance(selector,dict) and set(selector)=={'kind','country'}
              and selector.get('kind')=='border' and isinstance(selector.get('country'),str)
              and re.fullmatch('[A-Z0-9]{3}',selector['country']))
    explicit = (isinstance(provinces,list) and 0 < len(provinces) <= 512
                and all(type(n) is int and 0 < n <= 100000 for n in provinces)
                and len(set(provinces)) == len(provinces))
    if bool(border) == bool(explicit):
        return False
    p['target_level'] = target
    prefix = 'guide.fortifications.' + step['key']
    if ('province_id' in p and step.get('started_when') == {'op':'fact','key':'construction.queued',
            'test':'contains','value':f"province:{p['province_id']}:{p['building_id']}"}):
        del step['started_when']
    step.setdefault('started_when',{'op':'fact','key':prefix+'.started','test':'eq','value':True})
    step.setdefault('completion',{'op':'fact','key':prefix+'.completed','test':'eq','value':True})
    return True


def _value(state, key, *, observed=False):
    item = state['facts'].get(key,{})
    if (state['usable'] and item.get('source')
            and item.get('status') in (('observed',) if observed else ('observed','calculated'))):
        return item.get('value')
    return None


def context(guide,state):
    """Independent fixture oracle; never call the native evidence projector."""
    result=deepcopy(state);details={}
    def integer(value,low,high):return type(value) is int and low<=value<=high
    for rows in guide['tracks'].values():
        for step in rows:
            params=step['action'].get('params',{});key=step['key']
            prefix='guide.fortifications.'+key;building=params.get('building_id')
            if building not in BUILDINGS or step.get('completion')!={'op':'fact','key':prefix+'.completed','test':'eq','value':True}:
                continue
            selector=params.get('province_selector',{})
            target_country=selector.get('country') if isinstance(selector.get('country'),str) else ''
            provinces=(_value(result,'fortifications.borders.'+target_country)
                       if selector.get('kind')=='border' else params.get('province_ids',
                           [params['province_id']] if 'province_id' in params else None))
            target=params.get('target_level')
            valid=(integer(target,1,1000) and isinstance(provinces,list) and 1<=len(provinces)<=512
                   and all(integer(p,1,100000) for p in provinces) and len(set(provinces))==len(provinces))
            values=[_value(result,f'country.fortifications.items.{p}.{building}',observed=True) for p in provinces] if valid else []
            observed=valid and all(integer(v,0,1000) for v in values)
            done=all(v>=target for v in values) if observed else None
            queued=_value(result,'construction.queued')
            queue_known=isinstance(queued,list) and all(isinstance(v,str) for v in queued)
            started=((done is True or any(f'province:{p}:{building}' in queued for p in provinces))
                     if valid and queue_known else True if done is True else None)
            for suffix,value in (('started',started),('completed',done)):
                result['facts'][prefix+'.'+suffix]=(fact(value,'calculated','fortification_goal:'+key)
                    if value is not None else fact(reason='Provincial fortification evidence unavailable'))
            details[key]={'target_level':target if integer(target,1,1000) else None,
                          'province_ids':provinces if valid else [],
                          'completed_provinces':sum(v>=target for v in values) if observed else None,
                          'total_provinces':len(provinces) if valid else None,'building_id':building}
    return result,details


def enrich_borders(section, neighbors, country):
    result = deepcopy({k:v for k,v in section.items() if k != 'controllers'})
    result['borders_status'] = 'unavailable'
    controllers = section.get('controllers')
    rows = section.get('items')
    if section.get('status') != 'observed' or not isinstance(controllers,dict) or not isinstance(rows,list):
        return result
    borders = {}
    for row in rows:
        if not isinstance(row,dict):return result
        province = row.get('id')
        if not isinstance(neighbors.get(province),list) or controllers.get(str(province)) != country:
            return result
        for neighbor in neighbors[province]:
            owner = controllers.get(str(neighbor))
            if not isinstance(owner,str) or not re.fullmatch('[A-Z0-9]{3}',owner):
                return result
            if owner != country:
                borders.setdefault(owner,set()).add(province)
    result.update(borders_status='observed',borders={tag:sorted(ids) for tag,ids in sorted(borders.items())})
    return result
