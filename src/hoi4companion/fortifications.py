"""Generic province-level fortification goals; static topology is not ownership."""
from copy import deepcopy
import re
from .contracts import fact

BUILDINGS = {'bunker', 'coastal_bunker'}


def command(text,catalog):
    from .backend_core import _request
    from .resolution import catalog_index
    index=catalog_index(catalog)
    return _request(0,'import_fortification_command',text=text,**index.native_import_args())


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
    from .backend_core import _request
    return tuple(_request(0,'domain_context',kind='fortification',guide=guide,state=state))


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
