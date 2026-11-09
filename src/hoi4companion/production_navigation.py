"""Production HUD admission from qualified own-country controls."""
from .production_progress import _name
def matches(raw,proposals,rectangle):
    ui=raw.get('production_ui',{})
    if ui.get('status')!='observed' or raw.get('active_panel') in ('focus','research','event'):return []
    result=[]
    def variants(controls,target,*,picker=False):
        expected=target.get('production_selector',{}).get('design')
        if expected is not None:
            if not picker:
                # Runtime IDs already have qualified recipe membership. The
                # national/custom display name must not reject that identity.
                return controls
            from .design_matching import compare
            return [c for c in controls if compare(expected,c.get('design'))['status']=='match']
        names=target.get('production_selector',{}).get('variant_names',[])
        if not names:return controls
        normalized={_name(v) for v in names}
        return [c for c in controls if isinstance(c.get('variant_name'),str) and _name(c['variant_name']) in normalized]
    for target in proposals:
        if target.get('panel')!='production' or target.get('country')!=raw.get('country'):continue
        missing_product=target.get('production_missing') is True and target.get('goal_satisfied') is not True
        # Keep the add-category icon as a separate, qualified navigation cue.
        # It must coexist with a differing line or the exact picker product;
        # a factory-count shortfall on an already matching model needs no icon.
        if ui.get('visible') and missing_product:
            key=target.get('production_add_control')
            category=[c for c in ui.get('add',[]) if key and c.get('id')==key and c.get('visible') is True]
            if len(category)==1:
                rect=rectangle(category[0],{'clip':category[0].get('clip')})
                if rect is not None:
                    result.append({**target,'rect':rect,'navigation_stage':'add','basis':'native_gui_action_identity'})
        if not ui.get('visible'):
            if target.get('goal_satisfied') is True:continue
            controls=ui.get('access',[]);stage='access'
        elif ui.get('picker_open'):
            if ui.get('picker_count')!=1 or not missing_product:continue
            controls=variants([c for c in ui.get('equipment',[]) if c.get('equipment') in target.get('production_selector',{}).get('equipment_ids',[])],target,picker=True)
            stage='equipment'
        elif target.get('production_design_unverified') is True:
            ids=target.get('production_candidate_line_ids',[])
            if len(ids)!=1:continue
            controls=[c for c in ui.get('lines',[]) if c.get('line_id')==ids[0]
                      and c.get('equipment') in target.get('production_selector',{}).get('equipment_ids',[])]
            stage='design_candidate'
        elif missing_product:
            continue  # The category cue was emitted above; wait for its picker.
        else:
            ids=target.get('production_line_ids',[])
            # More than one matching line cannot authorize choosing one widget.
            if len(ids)!=1:continue
            controls=[c for c in ui.get('lines',[]) if c.get('line_id')==ids[0]
                      and c.get('equipment') in target.get('production_selector',{}).get('equipment_ids',[])]
            controls=variants(controls,target)
            stage='line'
        controls=[c for c in controls if c.get('visible') is True]
        if len(controls)!=1:continue
        control=controls[0];rect=rectangle(control,{'clip':control.get('clip')})
        if rect is None:continue
        result.append({**target,'rect':rect,'navigation_stage':stage,'basis':'native_gui_action_identity',
                       'title':'Apri Produzione' if stage=='access' else target['title']})
        # All admitted production instructions share the same entry point.
        if stage=='access':break
    grouped=[]
    for match in result:
        if match['navigation_stage']=='add':
            previous=next((p for p in grouped if p['navigation_stage']=='add' and p['rect']==match['rect']),None)
            if previous:
                if match['title'] not in previous['production_add_names']:previous['production_add_names'].append(match['title'])
                if match['key'] not in previous['step_keys']:previous['step_keys'].append(match['key'])
                continue
            match={**match,'production_add_names':[match['title']],'step_keys':[match['key']]}
        grouped.append(match)
    return grouped
