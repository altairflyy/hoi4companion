"""Pure production matching and assignment evidence, without native reads."""
import json
import unicodedata
from .contracts import fact
from .construction_progress import integer, MAX_INTEGER


def _name(value):
    return ''.join(c for c in unicodedata.normalize('NFKD',value).lower() if c.isalnum())


def selector_for(step):
    value=step['action'].get('params',{}).get('production_selector')
    if (not isinstance(value,dict) or set(value)!={'equipment_ids','variant_names','category'}
            or value['category'] not in (None,'military','naval')):
        return None
    for key,limit in [('equipment_ids',512),('variant_names',128)]:
        values=value[key]
        if (not isinstance(values,list) or len(values)>limit or any(not isinstance(v,str) or not v for v in values)
                or key=='equipment_ids' and not values): return None
    return {'category':value['category'],'equipment_ids':sorted(set(value['equipment_ids'])),
            'variant_names':sorted(set(value['variant_names']))}


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
                        yield {'key':step['key']+'.target-'+str(i+1),'action':{'kind':'manual','params':target}}
            else:
                yield step


def production_context(guide,state,baselines):
    facts,details=dict(state['facts']),{}
    lines=facts.get('production.lines',{})
    known=state['usable'] and lines.get('status') in ('observed','calculated') and bool(lines.get('source')) and isinstance(lines.get('value'),list)
    country=facts.get('country',{}).get('value') if state['usable'] else None
    for step in production_steps(guide):
            params=step['action'].get('params',{})
            if params.get('category')!='production' or 'production_selector' not in params: continue
            key='guide.production.'+step['key']; selector=selector_for(step)
            for suffix in ('assigned','requested','added','started'): facts[key+'.'+suffix]=fact(reason='Produzione non verificabile')
            matches=[]
            if known and selector:
                variants={_name(n) for n in selector['variant_names']}
                matches=[row for row in lines['value'] if row['equipment'] in selector['equipment_ids']
                         and (selector['category'] is None or row['category']==selector['category'])
                         and (not variants or _name(row['name']) in variants)]
            metrics={label:sum(int(row[field]) for row in matches) if known and selector else None
                     for label,field in [('assigned','assigned_factories'),('requested','requested_factories'),
                                         ('active','active_factories'),('damaged','damaged_factories')]}
            if any(v is not None and v>MAX_INTEGER for v in metrics.values()): metrics=dict.fromkeys(metrics)
            assigned,requested=metrics['assigned'],metrics['requested']
            requested_basis='requested_factories' in params
            count=requested if requested_basis else assigned
            target=params.get('requested_factories') if requested_basis else params.get('assigned_factories'); target=target if integer(target) else None
            spec=relative_spec(step); baseline=baselines.get(step['key']) if spec else None
            valid=(baseline is not None and baseline['country']==country and baseline['fact_key']==spec['fact_key']
                   and baseline['quantity']==spec['quantity'] and baseline['value']+spec['quantity']<=MAX_INTEGER)
            relative=params.get('quantity_mode')=='additional'
            initial=baseline['value'] if valid else None
            if relative: target=initial+spec['quantity'] if valid else None
            started=(requested>initial or assigned>initial) if relative and valid and assigned is not None else (
                None if relative or assigned is None else requested>0 or assigned>0)
            for suffix,value in [('assigned',assigned),('requested',requested),('added',count-initial if valid and count is not None else None),('started',started)]:
                facts[key+'.'+suffix]=fact(value,'calculated','production_adapter') if value is not None else fact(reason='Produzione o conteggio iniziale non verificabile')
            details[step['key']]={**metrics,'baseline':initial,'target':target,
                                 'lines':matches if assigned is not None else [],'basis':'requested' if requested_basis else 'assigned',
                                 'reason':'production_unavailable' if assigned is None else 'baseline_missing' if relative and not valid else ''}
    for rows in guide['tracks'].values():
        for step in rows:
            targets=step['action'].get('params',{}).get('production_targets')
            if isinstance(targets,list):
                details[step['key']]={'layout':[details.get(step['key']+'.target-'+str(i+1),{}) for i in range(len(targets))]}
    return {**state,'facts':facts},details
