"""Pure production matching and assignment evidence, without native reads."""
import json
import unicodedata
from .contracts import fact
from .construction_progress import integer, MAX_INTEGER


def _name(value):
    return ''.join(c for c in unicodedata.normalize('NFKD',value).lower() if c.isalnum() or c=='+')


def selector_for(step):
    value=step['action'].get('params',{}).get('production_selector')
    if (not isinstance(value,dict) or set(value)!={'equipment_ids','variant_names','category'}
            or value['category'] not in (None,'military','naval')):
        return None
    for key,limit in [('equipment_ids',512),('variant_names',128)]:
        values=value[key]
        if (not isinstance(values,list) or len(values)>limit or any(not isinstance(v,str) or not v for v in values)
                or key=='equipment_ids' and not values): return None
    result={'category':value['category'],'equipment_ids':sorted(set(value['equipment_ids'])),
            'variant_names':sorted(set(value['variant_names']))}
    params=step['action'].get('params',{})
    if 'design_reference' in params:
        from .design_matching import configuration
        recipe=configuration(params['design_reference'])
        if recipe is None:return None
        result['design']=recipe
    return result


def selector_identity(selector):
    """Match/revision identity; recipe-based matching never uses model labels.

    This is intentionally separate from the persisted relative baseline key.
    """
    value={k:v for k,v in selector.items() if k!='variant_names' or 'design' not in selector}
    return json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=False)


def relative_spec(step):
    params=step['action'].get('params',{}); selector=selector_for(step)
    requested='requested_factories' in params
    quantity=params.get('requested_factories') if requested else params.get('assigned_factories')
    if (selector is None or params.get('category')!='production' or params.get('quantity_mode')!='additional'
            or not integer(quantity,1) or step.get('completion')!={'op':'fact',
                'key':'guide.production.'+step['key']+'.added','test':'gte','value':quantity}): return None
    return {'fact_key':('production_requested:' if requested else 'production:')+json.dumps(selector,sort_keys=True,separators=(',',':'),ensure_ascii=False),
            'quantity':quantity}


def production_steps(guide):
    for rows in guide['tracks'].values():
        for step in rows:
            params=step['action'].get('params',{})
            targets=params.get('production_targets')
            if isinstance(targets,list):
                for i,target in enumerate(targets):
                    if isinstance(target,dict):
                        yield {'key':step['key']+'.target-'+str(i+1),'layout_key':step['key'],'action':{'kind':'manual','params':target}}
            else:
                yield step


def production_context(guide,state,baselines):
    from .backend_core import _request
    return tuple(_request(0,'production_context',kind='production',guide=guide,state=state,baselines=baselines))
