"""Admit action navigation from witnessed GUI controls, never screen guesses."""


def navigation_matches(raw, proposals, rectangle):
    observation=raw.get('action_ui',{})
    if observation.get('status')!='observed' or raw.get('active_panel') in ('focus','research','event'):
        return []
    controls=observation.get('controls',[]);panels=observation.get('panels',{})
    result=[]
    for target in proposals:
        panel=target.get('panel')
        if panel not in ('politics','projects') or target.get('country')!=raw.get('country'):continue
        from .action_readiness import hud_resource_ready
        if not hud_resource_ready(target.get('action_readiness')):continue
        required={'topbar','countrypoliticsview','political_ideas_window'} if panel=='politics' else {'topbar','countrytechnologyview','facilities_view','program_window'}
        if not required<=panels.keys():continue
        category=target.get('category_id');stage=None
        if panel=='politics':
            if panels.get('political_ideas_window'):
                # A selected category must be witnessed, not inferred from a
                # character's name or the order of slots in the GUI file.
                choices=[c for c in controls if c.get('root')=='political_ideas_window'
                         and c.get('entity_id')==target['identity']
                         and c.get('entity_kind')==target.get('entity_kind')
                         and c.get('category_id')==category and category]
                stage='entity'
            elif panels.get('countrypoliticsview'):
                choices=[c for c in controls if c.get('root')=='countrypoliticsview'
                         and category and c.get('category_id')==category
                         and c.get('name') in ('add_idea_button','idea_entry','open_roster_button','button')]
                stage='slot'
            else:
                choices=[c for c in controls if c.get('root')=='topbar' and c.get('name')=='player_flag']
                stage='access'
        else:
            if panels.get('program_window'):
                specialization=target.get('specialization')
                # The project list must belong to a program with a witnessed
                # matching specialization; an arbitrary land/naval icon is not proof.
                choices=[c for c in controls if c.get('root')=='program_window'
                         and specialization and c.get('specialization')==specialization
                         and c.get('entity_kind')=='project' and c.get('entity_id')==target['identity']]
                stage='entity'
            elif panels.get('facilities_view'):
                choices=[c for c in controls if c.get('root')=='facilities_view'
                         and target.get('specialization') and c.get('specialization')==target['specialization']
                         and c.get('name') in ('open_facility_button','facility_button')]
                stage='facility'
            elif panels.get('countrytechnologyview'):
                choices=[c for c in controls if c.get('root')=='countrytechnologyview' and c.get('name')=='facilities_tab']
                stage='access'
            else:
                choices=[c for c in controls if c.get('root')=='topbar' and c.get('name')=='technology_button']
                stage='access'
        choices=[c for c in choices if c.get('visible') is True]
        if len(choices)!=1:continue
        control=choices[0];rect=rectangle(control,{'clip':control.get('clip')})
        if rect is not None:
            caption={'access':'Apri Politica' if panel=='politics' else 'Apri Ricerca e progetti speciali','slot':'Apri lo slot '+str(category),'facility':'Apri l’impianto compatibile','entity':target.get('title',target['identity'])}[stage]
            result.append({**target,'title':caption,'rect':rect,'navigation_stage':stage,
                           'basis':'native_gui_action_identity'})
    return result
