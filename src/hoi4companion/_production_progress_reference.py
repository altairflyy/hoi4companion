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
        from ._design_matching_reference import configuration
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
    facts,details=dict(state['facts']),{}
    lines=facts.get('production.lines',{})
    known=state['usable'] and lines.get('status') in ('observed','calculated') and bool(lines.get('source')) and isinstance(lines.get('value'),list)
    country=facts.get('country',{}).get('value') if state['usable'] else None
    steps=list(production_steps(guide))
    # Snapshot-local only: new observations, recipes and campaigns must always
    # recompute matches. Sharing rows preserves their observed queue identity.
    matches_cache={}
    military=[row for row in lines['value'] if row['category']=='military'] if known else []
    selectors={step['key']:selector_for(step) for step in steps
               if step['action'].get('params',{}).get('category')=='production'}
    priority_peers={};priority_allowed={}
    for step in steps:
        priority=step['action'].get('params',{}).get('production_priority')
        if priority:
            priority_peers.setdefault((step.get('layout_key',step['key']),priority),[]).append(selectors.get(step['key']))
    def comparison(selector):
        if not known or not selector:return [],True,None
        key=selector_identity(selector)
        if key not in matches_cache:
            from ._design_matching_reference import matching as match_design
            matches_cache[key]=match_design(selector,lines['value'])
        return matches_cache[key]
    for step in steps:
            params=step['action'].get('params',{})
            if params.get('category')!='production' or 'production_selector' not in params: continue
            key='guide.production.'+step['key']; selector=selectors[step['key']]
            for suffix in ('assigned','requested','added','started','remaining_allocated'): facts[key+'.'+suffix]=fact(reason='Produzione non verificabile')
            matches,configuration_known,design_check=comparison(selector)
            metrics={label:sum(int(row[field]) for row in matches) if known and selector and configuration_known else None
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
            remaining=params.get('quantity_mode')=='remaining'
            allocated=None; remaining_reason=''
            if remaining:
                total_fact=facts.get('country.production.effective_factories.military',{})
                total=total_fact.get('value') if (state['usable'] and total_fact.get('status') in ('observed','calculated')
                                                and total_fact.get('source')) else None
                if not integer(total):total=None
                target=None
                if known and selector and configuration_known and selector['category']=='military' and total is not None:
                    others=sum(row['requested_factories'] for row in military if row not in matches)
                    if others<=MAX_INTEGER:
                        target=max(total-others,0)
                        # Counts prove the current configuration, not the game's
                        # unobserved automatic-allocation preference. A last line
                        # requesting spare capacity can absorb future factories.
                        allocated=(len(matches)==1 and bool(military) and matches[0] is military[-1]
                                   and requested>0 and requested>=target)
                if target is None:remaining_reason='remaining_capacity_unknown'
                elif len(matches)>1:remaining_reason='remaining_line_ambiguous'
                elif not matches:remaining_reason='remaining_line_missing'
                elif matches[0] is not military[-1]:remaining_reason='remaining_line_not_last'
            started=(requested>initial or assigned>initial) if relative and valid and assigned is not None else (
                None if relative or assigned is None else requested>0 or assigned>0)
            priority=params.get('production_priority');priority_value=None
            if priority and known and selector and configuration_known and selector['category']=='military':
                group=(step.get('layout_key',step['key']),priority)
                if group not in priority_allowed:
                    allowed=set();uncertain=set()
                    for peer in priority_peers[group]:
                        rows,complete,check=comparison(peer)
                        allowed.update(r['id'] for r in rows)
                        if not complete and check:
                            uncertain.update(r['line_id'] for r in check['candidates'] if r['status']=='unknown')
                    priority_allowed[group]=(allowed,uncertain)
                allowed,uncertain=priority_allowed[group]
                priority_value=False
                if len(matches)==1:
                    position=military.index(matches[0])
                    surrounding=military[:position] if priority=='first' else military[position+1:]
                    values=[True if r['id'] in allowed else None if r['id'] in uncertain else False for r in surrounding]
                    priority_value=False if False in values else None if None in values else True
            if priority:facts[key+'.priority_satisfied']=fact(priority_value,'calculated','production_adapter') if priority_value is not None else fact(reason='Production priority unavailable')
            for suffix,value in [('assigned',assigned),('requested',requested),('added',count-initial if valid and count is not None else None),('started',started),('remaining_allocated',allocated)]:
                facts[key+'.'+suffix]=fact(value,'calculated','production_adapter') if value is not None else fact(reason='Produzione o conteggio iniziale non verificabile')
            details[step['key']]={**metrics,'baseline':initial,'target':target,
                                 'lines':matches if assigned is not None else [],'basis':'remaining' if remaining else 'requested' if requested_basis else 'assigned',
                                 'reason':remaining_reason if remaining else 'production_unavailable' if assigned is None else 'baseline_missing' if relative and not valid else ''}
            if priority:details[step['key']].update(priority=priority,priority_satisfied=priority_value)
            if design_check:
                from copy import deepcopy
                details[step['key']]['design_check']=deepcopy(design_check)
    for rows in guide['tracks'].values():
        for step in rows:
            targets=step['action'].get('params',{}).get('production_targets')
            if isinstance(targets,list):
                details[step['key']]={'layout':[details.get(step['key']+'.target-'+str(i+1),{}) for i in range(len(targets))]}
    return {**state,'facts':facts},details
