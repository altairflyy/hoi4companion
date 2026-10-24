"""Connect structured imported orders to existing facts; never acquire live data."""
from pathlib import Path
import re
from copy import deepcopy

from .clausewitz import Node
from .jomini_parser import parse_file
from .installation import collect_files
from .research_upgrades import exact_matches, normal_name
from .tooltips import localise
from .construction_progress import BUILDING_FACTS
from .political_import import political_catalog, bind_political


def operation_catalog(game, mods, labels, reference):
    result, warnings = [], []
    country_names={}
    for texts in (reference,labels):
        for key,text in texts.items():
            match=re.fullmatch(r'[A-Z0-9]{3}_(.+)',key)
            if match: country_names.setdefault(match[1],[]).append(localise(text,texts))
    def names(key):
        english = localise(reference.get(key,key),reference)
        display = localise(labels.get(key,english),labels)
        aliases=list(dict.fromkeys(text for text in [display,*country_names.get(key,[])] if text!=english))
        return {'name':english,'localized_name':display,'aliases':aliases}
    for directory,kind in [('common/buildings','building'),('history/states','state')]:
        files, issues = collect_files(game,mods,directory,'.txt'); warnings.extend(issues)
        for path in files:
            try:
                root=parse_file(path)
                if kind=='building':
                    for group in root.blocks('buildings'):
                        result.extend({'kind':kind,'id':key,**names(key)}
                                      for key,node in group.entries if isinstance(node,Node))
                else:
                    for state in root.blocks('state'):
                        key=state.scalar('id')
                        if key.isdigit() and int(key)>0:
                            result.append({'kind':kind,'id':str(int(key)),**names(state.scalar('name','STATE_'+key))})
            except (OSError,ValueError,RecursionError) as exc:
                warnings.append(f'Catalogo operazioni non leggibile ({Path(path).name}): {exc}')
    equipment_files, issues = collect_files(game,mods,'common/units/equipment','.txt'); warnings.extend(issues)
    definitions,duplicates = {},[]
    for path in equipment_files:
        try:
            parsed=parse_file(path)
            for group in parsed.blocks('equipments'):
                definitions.update((key,node) for key,node in group.entries if isinstance(node,Node))
            duplicates.extend((key,node) for group in parsed.blocks('duplicate_archetypes') for key,node in group.entries if isinstance(node,Node))
        except (OSError,ValueError,RecursionError) as exc:
            warnings.append(f'Catalogo equipaggiamenti non leggibile ({path.name}): {exc}')
    # Role-specific airframes and chassis can be declared by duplicate_archetypes
    # instead of equipments. Preserve explicitly documented variant IDs; do not
    # guess a model from its display name or a dynamic live variant name.
    for key,node in duplicates:
        if node.scalar('archetype') not in definitions:continue
        definitions.setdefault(key,Node([('is_archetype','yes')]))
        for block in node.blocks('variant_name'):
            for identifier,label in block.entries:
                if isinstance(label,str):definitions.setdefault(identifier,Node([('archetype',key),('derived_variant_name',label)]))
    country_aliases = {}
    for texts in (reference,labels):
        for key,text in texts.items():
            if len(key)<5 or key[3]!='_' or not re.fullmatch('[A-Z0-9]{3}',key[:3]): continue
            identifier=key[4:]
            if identifier not in definitions and identifier.endswith('_short'): identifier=identifier[:-6]
            if identifier in definitions:
                country_aliases.setdefault(identifier,{}).setdefault(key[:3],[]).append(localise(text,texts))
    def family(identifier):
        seen=set()
        while identifier in definitions and identifier not in seen:
            seen.add(identifier); parent=definitions[identifier].scalar('archetype')
            if not parent: return identifier
            identifier=parent
        return None
    members={}
    for key,node in definitions.items():
        root=family(key)
        if root and node.scalar('is_archetype')!='yes': members.setdefault(root,[]).append(key)
    for key,node in definitions.items():
        root=family(key)
        if root is None: continue
        label=names(key)
        label['aliases']=list(dict.fromkeys([*label['aliases'],
            *[localise(texts[node.scalar('derived_variant_name')],texts) for texts in (reference,labels) if node.scalar('derived_variant_name') in texts],
            *[localise(texts[key+'_short'],texts) for texts in (reference,labels) if key+'_short' in texts],
            key.removesuffix('_equipment').replace('_',' '), label['name']+'s']))
        result.append({'kind':'equipment','id':key,**label,'family_id':root,
                       'equipment_ids':sorted(members.get(root,[])) if node.scalar('is_archetype')=='yes' else [key],
                       'is_archetype':node.scalar('is_archetype')=='yes',
                       'country_aliases':{country:list(dict.fromkeys(items)) for country,items in country_aliases.get(key,{}).items()}})
    project_files,issues=collect_files(game,mods,'common/special_projects/projects','.txt');warnings.extend(issues)
    for path in project_files:
        try:
            result.extend({'kind':'project','id':key,**names(key)} for key,node in parse_file(path).entries if isinstance(node,Node))
        except (OSError,ValueError,RecursionError) as exc:
            warnings.append(f'Catalogo progetti non leggibile ({path.name}): {exc}')
    political,issues=political_catalog(game,mods,labels,reference)
    result.extend(political); warnings.extend(issues)
    return result,warnings


def _fact(key,test,value):
    return {'op':'fact','key':key,'test':test,'value':value}


def _automatic(condition):
    if not isinstance(condition,dict): return False
    if condition.get('op')=='fact': return True
    if condition.get('op') in ('all','any'): return all(_automatic(item) for item in condition['args'])
    if condition.get('op')=='not': return _automatic(condition['arg'])
    return False


def _integer(value,minimum=0):
    return type(value) is int and value>=minimum


def bind_operations(guide,catalog):
    reports=[]
    variants={}
    country=guide.get('applies_to',{}).get('country')
    # A guide can explicitly name a custom model, then refer to that same model
    # in later assignments. Reuse only unique, declared type/model pairs.
    for rows in guide['tracks'].values():
        for row in rows:
            params=row['action'].get('params',{})
            if params.get('category')=='production' and isinstance(params.get('variant_name'),str):
                probe=deepcopy(row)
                _bind_production(probe,catalog,country)
                selector=probe['action'].get('params',{}).get('production_selector')
                if selector:
                    matches=variants.setdefault(normal_name(params['variant_name']),[])
                    if selector not in matches: matches.append(selector)
    def match(kind,identifier,name):
        rows=[entry for entry in catalog if entry['kind']==kind]
        query=str(identifier) if identifier is not None else name
        if not isinstance(query,str) or not query: return None
        matches=exact_matches(query,rows)
        return matches[0] if len(matches)==1 else None
    for rows in guide['tracks'].values():
        for step in rows:
            if step['action']['kind'] not in ('manual','special_project'): continue
            if step.get('tracking')=='reminder':
                reports.append({'key':step['key'],'title':step.get('title',step['key']),
                                'monitoring':'reminder','automatic_start':False,'automatic_completion':False,
                                'reason':'method_reminder'})
                continue
            params=step['action'].get('params',{})
            reason='no_supported_live_binding'
            agency=params.get('operation_id')=='create_intelligence_agency' or (normal_name(str(params.get('project_name',''))) in ('intelligenceagency','agenziadiintelligence') and params.get('target_quantity') in (None,1) and not params.get('upgrade_count'))
            if agency:
                params['operation_id']='create_intelligence_agency'
                created=_fact('country.intelligence.is_created','eq',True)
                step.setdefault('started_when',{'op':'any','args':[_fact('country.intelligence.in_creation','eq',True),created]})
                step.setdefault('completion',created)
                reason='intelligence_creation_flags'
            elif params.get('category')=='projects' and (params.get('project_id') or params.get('project_name')) or step['action']['kind']=='special_project':
                ref=step['action'].get('entity',{})
                project=match('project',params.get('project_id') or ref.get('id'),params.get('project_name'))
                reason='project_identity_unresolved'
                if project:
                    params['project_id']=project['id']
                    step['action'].update(kind='special_project',entity={'kind':'project','id':project['id']})
                    step['action'].setdefault('params',params)
                    done=_fact('projects.completed','contains',project['id'])
                    step.setdefault('started_when',{'op':'any','args':[_fact('projects.active','contains',project['id']),done]})
                    step.setdefault('completion',done)
                    reason='project_live_state'
            elif params.get('category')=='construction':
                building=match('building',params.get('building_id'),params.get('building_name'))
                region=match('state',params.get('state_id'),params.get('state_name'))
                reason='building_or_state_not_resolved'
                if building and not region and not params.get('state_id') and not params.get('state_name'):
                    params['building_id']=building['id']
                    step.setdefault('started_when',_fact('construction.queued','contains','building:'+building['id']))
                    reason='building_start_without_location'
                if building and region:
                    params['building_id'],params['state_id']=building['id'],int(region['id'])
                    step.setdefault('started_when',_fact('construction.queued','contains',f"state:{region['id']}:{building['id']}"))
                    target=params.get('target_level')
                    if not _integer(target,1): target=params.get('target_quantity')
                    if not _integer(target,1):
                        if params.get('quantity_mode')=='target_total': target=params.get('quantity')
                        elif params.get('quantity_mode')=='additional' and _integer(params.get('baseline_level')) and _integer(params.get('quantity'),1):
                            target=params['baseline_level']+params['quantity']
                    field=BUILDING_FACTS.get(building['id'])
                    reason='additional_baseline_missing' if params.get('quantity_mode')=='additional' else 'target_not_defined'
                    if not field: reason='building_completion_not_observed'
                    elif _integer(target,1):
                        step.setdefault('completion',_fact(f"country.states.items.{region['id']}.{field}",'gte',target))
                        reason='observed_building_level'
                    elif params.get('quantity_mode')=='additional' and _integer(params.get('quantity'),1):
                        step.setdefault('completion',_fact('guide.construction.'+step['key']+'.added','gte',params['quantity']))
                        reason='baseline_from_live'
            elif params.get('category')=='production':
                reason = _bind_production(step,catalog,country,variants)
            else:
                reason=bind_political(step,catalog,country) or reason
            started,completed=_automatic(step.get('started_when')),_automatic(step.get('completion'))
            if started and not completed and 'completion' in step: reason='declared_manual_completion'
            reports.append({'key':step['key'],'title':step.get('title',step['key']),
                            'monitoring':'automatic' if completed else 'start_only' if started else 'manual',
                            'automatic_start':started,'automatic_completion':completed,'reason':reason})
    return reports


def _bind_production(step,catalog,country,known_variants=None):
    params=step['action'].get('params',{})
    targets=params.get('production_targets')
    if targets is not None:
        for field,op in [('completion','all'),('started_when','any')]:
            condition=step.get(field,{})
            if condition.get('op')==op and all(isinstance(arg,dict) and arg.get('op')=='fact'
                    and arg.get('key','').startswith('guide.production.'+step['key']+'.target-')
                    for arg in condition.get('args',[])):
                step.pop(field)
        if not isinstance(targets,list) or not 0<len(targets)<=64 or any(not isinstance(p,dict) or 'production_targets' in p for p in targets):
            return 'production_targets_invalid'
        if any(p.get('quantity_mode')=='additional' for p in targets):return 'production_layout_requires_totals'
        children=[{'key':step['key']+'.target-'+str(i+1),'action':{'kind':'manual','params':{**p,'category':'production'}}} for i,p in enumerate(targets)]
        reasons=[_bind_production(child,catalog,country,known_variants) for child in children]
        params['production_targets']=[child['action']['params'] for child in children]
        if all('completion' in child for child in children):
            step.setdefault('completion',{'op':'all','args':[child['completion'] for child in children]})
        if all('started_when' in child for child in children):
            step.setdefault('started_when',{'op':'any','args':[child['started_when'] for child in children]})
        return 'production_layout' if all('completion' in child for child in children) else next((r for r in reasons if r not in ('production_live_assignment','production_relative_assignment')),'production_target_missing')
    if 'production_selector' in params:
        prefix='guide.production.'+step['key']
        for field,keys in [('completion',(prefix+'.assigned',prefix+'.requested',prefix+'.added')),('started_when',(prefix+'.started',))]:
            condition=step.get(field,{})
            if condition.get('op')=='fact' and condition.get('key') in keys: step.pop(field)
        params.pop('production_selector')
    if params.get('mio_name') is not None: return 'production_mio_not_observed'
    entries=[{**e,'aliases':[*e.get('aliases',[]),*e.get('country_aliases',{}).get(country,[])]}
             for e in catalog if e['kind']=='equipment']
    query=params.get('equipment_id') if params.get('equipment_id') is not None else params.get('equipment_name')
    if not isinstance(query,str) or not query: return 'production_equipment_unresolved'
    matches=[e for e in entries if e['id']==query] if params.get('equipment_id') is not None else exact_matches(query,entries)
    # Generic labels shared by a family and its first level denote the family.
    roots=[e for e in matches if e.get('is_archetype')]
    if len(roots)==1 and all(e.get('family_id')==roots[0]['id'] for e in matches): matches=roots
    declared=(known_variants or {}).get(normal_name(query),[]) if params.get('equipment_id') is None else []
    if not matches and len(declared)>1: return 'production_equipment_ambiguous'
    if not matches and len(declared)==1:
        selector=deepcopy(declared[0]); identifiers=selector['equipment_ids']; variants=selector['variant_names']
    elif not matches: return 'production_equipment_unresolved'
    else:
        if len(matches)!=1: return 'production_equipment_ambiguous'
        equipment=matches[0]; identifiers=equipment.get('equipment_ids',[equipment['id']]); variants=[]
    variant=params.get('variant_name')
    if variant is not None:
        if not isinstance(variant,str) or not variant.strip() or ';' in variant: return 'production_variant_ambiguous'
        variants=[variant]
        named=exact_matches(variant,entries)
        if len(named)>1: return 'production_variant_ambiguous'
        if named:
            chosen=named[0]; selected=chosen.get('equipment_ids',[chosen['id']])
            if not set(selected)<=set(identifiers): return 'production_variant_incompatible'
            identifiers=selected
            variants=list(dict.fromkeys([variant,chosen['name'],chosen.get('localized_name',chosen['name']),*chosen.get('aliases',[])]))
    category=params.get('production_category')
    if category not in (None,'military','naval'): return 'production_category_invalid'
    if not identifiers: return 'production_equipment_unresolved'
    params['production_selector']={'equipment_ids':sorted(identifiers),'variant_names':sorted(variants),'category':category}
    key='guide.production.'+step['key']
    step.setdefault('started_when',_fact(key+'.started','eq',True))
    requested='requested_factories' in params
    target=params.get('requested_factories') if requested else params.get('assigned_factories')
    if requested and 'assigned_factories' in params:return 'production_target_invalid'
    if not _integer(target): return 'production_target_missing'
    relative=params.get('quantity_mode')=='additional'
    if relative and target==0: return 'production_target_missing'
    test='gte' if relative else params.get('assignment_test','eq')
    if test not in ('eq','gte'): return 'production_target_invalid'
    step.setdefault('completion',_fact(key+('.added' if relative else '.requested' if requested else '.assigned'),test,target))
    return 'production_relative_assignment' if relative else 'production_live_assignment'
