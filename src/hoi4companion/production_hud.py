"""Project admitted production instructions; never choose a strategy or widget."""
from .production_progress import selector_for

ADD_CONTROLS={'interface_category_land':'infantry_button','interface_category_armor':'tank_button',
              'interface_category_air':'aircraft_button','interface_category_capital_ships':'naval_button',
              'interface_category_screen_ships':'naval_button','interface_category_other_ships':'naval_button'}


def proposals(row,item,catalog,country,state=None,*,equipment=None):
    params=row['action'].get('params',{})
    children=params.get('production_targets')
    layouts=item.get('production',{}).get('layout',[])
    entries=[(p,layouts[i] if i<len(layouts) else {},'.target-'+str(i+1)) for i,p in enumerate(children)] if isinstance(children,list) else [(params,item.get('production',{}),'')]
    result=[]
    if equipment is None:equipment=[e for e in catalog if e['kind']=='equipment']
    for p,detail,suffix in entries:
        if detail.get('superseded_by'):continue
        selector=selector_for({'action':{'params':p}})
        if not selector:continue
        target=detail.get('target');basis=detail.get('basis');count=detail.get(basis)
        if target is None and p.get('quantity_mode') not in ('additional','remaining'):
            target=p.get('requested_factories',p.get('assigned_factories'))
        numeric=type(target) in (int,float) and type(count) in (int,float)
        comparison='gte' if p.get('quantity_mode')=='additional' else p.get('assignment_test','eq')
        satisfied=(count==target if comparison=='eq' else count>=target) if numeric else None
        if basis=='remaining':
            satisfied=(detail['requested']>0 and detail['requested']>=target and not detail.get('reason')) if type(target) in (int,float) and type(detail.get('requested')) in (int,float) else None
        if p.get('production_priority'):
            priority=detail.get('priority_satisfied')
            satisfied=False if priority is False or satisfied is False else None if priority is not True else satisfied
        names=[]
        reference=p.get('design_reference',{})
        if selector['variant_names']:names=[reference.get('name') or p.get('variant_name')]
        catalog_rows=[e for e in equipment if e['id']==(p.get('equipment_id') or p.get('equipment_name'))]
        if not catalog_rows:
            roots=[e for e in equipment if e.get('is_archetype') and set(e.get('equipment_ids',[]))==set(selector['equipment_ids'])]
            if len(roots)==1:catalog_rows=roots
        if not names:names=[n for e in catalog_rows for n in (e.get('localized_name'),e.get('name')) if n]
        names=[n for n in names if isinstance(n,str) and n]
        if not names:names=[p.get('variant_name') or p.get('equipment_name') or p.get('equipment_id')]
        names=list(dict.fromkeys(n for n in names if isinstance(n,str) and n))
        if not names:continue
        routing={e['id']:ADD_CONTROLS.get(e.get('production_ui_category')) for e in catalog if e['kind']=='equipment' and e['id'] in selector['equipment_ids']}
        controls={routing.get(identifier) for identifier in selector['equipment_ids']}
        add_control=next(iter(controls)) if len(controls)==1 and None not in controls else None
        known=type(detail.get('requested')) is int and type(detail.get('assigned')) is int
        missing=not detail.get('lines') if known else None
        proposal={'key':row['key']+suffix,'guide_step':row['key'],'panel':'production',
            'identity':p.get('variant_name') or p.get('equipment_id') or p.get('equipment_name'),
            'title':names[0],'labels':names,'status':item['status'],'country':country,
            'quantity':target,'quantity_mode':p.get('quantity_mode','total'),'production_basis':basis,
            'requested':detail.get('requested'),'assigned':detail.get('assigned'),
            'goal_satisfied':satisfied,'production_missing':missing,'production_add_control':add_control,
            # The existing remaining-capacity policy requires the final line.
            'production_priority':p.get('production_priority') or ('last' if p.get('quantity_mode')=='remaining' else None),
            'production_selector':selector,
            'production_line_ids':[str(line['id']) for line in detail.get('lines',[]) if isinstance(line.get('id'),str)]}
        check=detail.get('design_check')
        if check:
            proposal['production_design_check']=check
            candidates=check.get('candidates',[])
            if check.get('status') in ('different','unknown') and len(candidates)==1:
                candidate=candidates[0]
                proposal.update(production_design_unverified=True,goal_satisfied=None,
                    production_candidate_line_ids=[candidate['line_id']],
                    production_candidate_name=candidate['name'])
        if 'design' not in selector and selector['variant_names'] and missing is True and state is not None:
            from .contracts import normalise_state
            normalized=normalise_state(state);observed=normalized['facts'].get('production.lines',{})
            if (normalized['usable'] and normalized['facts'].get('country',{}).get('value')==country
                    and observed.get('status') in ('observed','calculated') and observed.get('source')):
                candidates=[line for line in observed.get('value',[]) if line['equipment'] in selector['equipment_ids']
                            and selector['category'] in (None,line['category'])]
                # This is a design-verification cue, never a recipe match or
                # a reason to admit a pending step. No candidate counts enter
                # the guide's completion or remaining-capacity calculations.
                if len(candidates)==1:
                    candidate=candidates[0]
                    proposal.update(production_design_unverified=True,goal_satisfied=None,
                        production_candidate_line_ids=[candidate['id']],
                        production_candidate_name=candidate['name'])
        result.append(proposal)
    return result
