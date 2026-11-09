"""Connect structured imported orders to existing facts; never acquire live data."""
from pathlib import Path
import re
from copy import deepcopy

from .clausewitz import Node
from .jomini_parser import parse_file
from .installation import collect_files
from .research_upgrades import normal_name
from .tooltips import localise
from .construction_progress import BUILDING_FACTS
from ._political_import_reference import political_catalog, bind_political
from .geography import state_geography, building_scope
from .resolution import catalog_index, ResolutionContext


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
                        for key,node in group.entries:
                            if isinstance(node,Node):
                                scope=building_scope(node)
                                cap=next((n.scalar('province_max') for n in node.blocks('level_cap') if n.scalar('province_max').isdigit()),'')
                                result.append({'kind':kind,'id':key,**names(key),
                                               **({'location_scope':scope} if scope else {}),
                                               **({'max_level':int(cap)} if cap else {})})
                else:
                    for state in root.blocks('state'):
                        key=state.scalar('id')
                        if key.isdigit() and int(key)>0:
                            result.append({'kind':kind,'id':str(int(key)),**names(state.scalar('name','STATE_'+key)),
                                           'geography':state_geography(state,names)})
            except (OSError,ValueError,RecursionError) as exc:
                warnings.append(f'Catalogo operazioni non leggibile ({Path(path).name}): {exc}')
    from ._map_topology_reference import load_neighbors
    try:
        neighbors = load_neighbors(game, mods)
    except (OSError,ValueError,KeyError) as exc:
        neighbors = {}
        warnings.append('Province topology unavailable: ' + str(exc))
    for region in result:
        if region['kind']=='state' and neighbors:
            geo=region['geography']
            geo['province_neighbors']={str(p):neighbors[p] for p in geo['province_ids'] if p in neighbors}
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
    def interface_category(identifier):
        seen=set()
        while identifier in definitions and identifier not in seen:
            seen.add(identifier);node=definitions[identifier]
            category=node.scalar('interface_category')
            if category:return category
            identifier=node.scalar('archetype')
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
                       'production_ui_category':interface_category(key),
                       'equipment_ids':sorted(members.get(root,[])) if node.scalar('is_archetype')=='yes' else [key],
                       'is_archetype':node.scalar('is_archetype')=='yes',
                       'country_aliases':{country:list(dict.fromkeys(items)) for country,items in country_aliases.get(key,{}).items()}})
    project_files,issues=collect_files(game,mods,'common/special_projects/projects','.txt');warnings.extend(issues)
    for path in project_files:
        try:
            from .action_resources import project_costs
            result.extend({'kind':'project','id':key,**names(key), 'specialization':node.scalar('specialization'),
                           'resource_costs':project_costs(node)} for key,node in parse_file(path).entries if isinstance(node,Node))
        except (OSError,ValueError,RecursionError) as exc:
            warnings.append(f'Catalogo progetti non leggibile ({path.name}): {exc}')
    political,issues=political_catalog(game,mods,labels,reference)
    result.extend(political); warnings.extend(issues)
    from ._doctrines_reference import doctrine_catalog
    doctrines,issues=doctrine_catalog(game,mods,labels,reference)
    result.extend(doctrines);warnings.extend(issues)
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
    catalog=catalog_index(catalog)
    geography=catalog.geography
    from .reference_designs import bindings,name_key
    variants=bindings(guide,catalog)
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
                    matches=variants.setdefault(name_key(params['variant_name']),[])
                    if selector not in matches: matches.append(selector)
    def match(kind,identifier,name):
        query=str(identifier) if identifier is not None else name
        if not isinstance(query,str) or not query: return None
        matches=catalog.matches(query,(kind,),explicit=identifier is not None)
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
            from .foreign_interactions import bind as bind_foreign
            if bind_foreign(step,catalog):
                reports.append({'key':step['key'],'title':step.get('title',step['key']),
                                'monitoring':'automatic' if _automatic(step.get('completion')) else 'manual',
                                'automatic_start':_automatic(step.get('started_when')),'automatic_completion':_automatic(step.get('completion')),
                                'reason':'foreign_interaction_live_state'})
                continue
            location=None
            bindings=[]
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
                provincial=building is not None and building.get('location_scope')=='province'
                location=geography.resolve(params,provincial)
                region=location.get('state_id')
                reason='building_or_state_not_resolved'
                from .fortifications import bind as bind_fortification
                has_group=any(k in params for k in ('province_selector','province_ids'))
                group = provincial and building['id'] in ('bunker','coastal_bunker') and has_group
                invalid_group=has_group and not group
                if invalid_group:
                    reason='province_selector_requires_provincial_fort'
                elif group:
                    params['building_id']=building['id']
                    ids=params.get('province_ids')
                    known_provinces=(not isinstance(ids,list) or not geography.provinces or
                        all(len(geography.provinces.get(n,()))==1 and (not region or region in geography.provinces[n]) for n in ids if type(n) is int))
                    if known_provinces and bind_fortification(step,catalog):
                        reason='provincial_fortification_goal'
                    else:
                        reason='fortification_target_unresolved'
                elif building and not region and all(params.get(k) is None for k in ('state_id','state_name','province_id')):
                    params['building_id']=building['id']
                    step.setdefault('started_when',_fact('construction.queued','contains','building:'+building['id']))
                    reason='building_start_without_location'
                elif building and not region:
                    reason=location['reason']
                if building and region and not group and not invalid_group:
                    params['building_id'],params['state_id']=building['id'],region
                    if provincial:params['province_id']=location['province_id']
                    queue=f"province:{params['province_id']}:{building['id']}" if provincial else f"state:{region}:{building['id']}"
                    step.setdefault('started_when',_fact('construction.queued','contains',queue))
                    target=params.get('target_level')
                    if not _integer(target,1): target=params.get('target_quantity')
                    if not _integer(target,1):
                        if params.get('quantity_mode')=='target_total': target=params.get('quantity')
                        elif params.get('quantity_mode')=='additional' and _integer(params.get('baseline_level')) and _integer(params.get('quantity'),1):
                            target=params['baseline_level']+params['quantity']
                    field=BUILDING_FACTS.get(building['id']) if not provincial else None
                    reason='additional_baseline_missing' if params.get('quantity_mode')=='additional' else 'target_not_defined'
                    if not field: reason='building_completion_not_observed'
                    elif _integer(target,1):
                        step.setdefault('completion',_fact(f"country.states.items.{region}.{field}",'gte',target))
                        reason='observed_building_level'
                    elif params.get('quantity_mode')=='additional' and _integer(params.get('quantity'),1):
                        step.setdefault('completion',_fact('guide.construction.'+step['key']+'.added','gte',params['quantity']))
                        reason='baseline_from_live'
                    if provincial and building['id'] in ('bunker','coastal_bunker') and params.get('quantity_mode')=='additional' and not _integer(target,1) and _integer(params.get('quantity'),1):
                        step.setdefault('completion',_fact('guide.construction.'+step['key']+'.added','gte',params['quantity']))
                        reason='baseline_from_live'
                    if provincial and bind_fortification(step,catalog):
                        reason='provincial_fortification_goal'
            elif params.get('category')=='production':
                reason = _bind_production(step,catalog,country,variants)
            elif params.get('doctrine_id') or params.get('doctrine_name'):
                from .doctrines import bind_doctrine
                reason=bind_doctrine(step,catalog,country)
            else:
                reason=bind_political(step,catalog,country,audit=bindings) or reason
            started,completed=_automatic(step.get('started_when')),_automatic(step.get('completion'))
            if reason=='doctrine_selection_unobserved':completed=False
            if (reason=='decision_identity_resolved' and step.get('completion')==
                    _fact('decisions.executed','contains',params.get('decision_id'))):
                # The contract can express execution, but the current native
                # provider publishes only listed decisions, not their history.
                completed=False
            if started and not completed and 'completion' in step: reason='declared_manual_completion'
            reports.append({'key':step['key'],'title':step.get('title',step['key']),
                            'monitoring':'automatic' if completed else 'start_only' if started else 'manual',
                            'automatic_start':started,'automatic_completion':completed,'reason':reason,
                            **({'bindings':bindings} if bindings else {}),
                            **({'location':location} if location and 'state_id' in location else {})})
    # Freshly promoted appointments/doctrines receive their qualified
    # completion predicates above. Only now can their source-order milestones
    # be attached; this is also needed for first-time Word imports.
    from .production_milestones import annotate as annotate_milestones
    annotate_milestones(guide)
    return reports


def _bind_production(step,catalog,country,known_variants=None):
    catalog=catalog_index(catalog)
    params=step['action'].get('params',{})
    targets=params.get('production_targets')
    def owned(node,prefix):
        if not isinstance(node,dict):return False
        if node.get('op')=='fact':
            key=node.get('key','')
            return key.startswith(prefix) and re.fullmatch(r'(?:\d+\.)?(?:assigned|requested|added|remaining_allocated|priority_satisfied|started)',key[len(prefix):]) is not None
        return node.get('op') in ('all','any') and bool(node.get('args')) and all(owned(n,prefix) for n in node['args'])
    if targets is not None:
        for field,op in [('completion','all'),('started_when','any')]:
            condition=step.get(field,{})
            if condition.get('op')==op and owned(condition,'guide.production.'+step['key']+'.target-'):
                step.pop(field)
        if not isinstance(targets,list) or not 0<len(targets)<=64 or any(not isinstance(p,dict) or 'production_targets' in p for p in targets):
            return 'production_targets_invalid'
        if any(p.get('quantity_mode')=='additional' for p in targets):return 'production_layout_requires_totals'
        remaining=[i for i,p in enumerate(targets) if p.get('quantity_mode')=='remaining']
        if remaining and remaining!=[len(targets)-1]:return 'production_targets_invalid'
        children=[{'key':step['key']+'.target-'+str(i+1),'action':{'kind':'manual','params':{**p,'category':'production'}}} for i,p in enumerate(targets)]
        reasons=[_bind_production(child,catalog,country,known_variants) for child in children]
        params['production_targets']=[child['action']['params'] for child in children]
        if remaining:
            selected=set(children[-1]['action']['params'].get('production_selector',{}).get('equipment_ids',[]))
            if any(selected.intersection(child['action']['params'].get('production_selector',{}).get('equipment_ids',[]))
                   for child in children[:-1]):return 'production_targets_invalid'
        if all('completion' in child for child in children):
            step.setdefault('completion',{'op':'all','args':[child['completion'] for child in children]})
        if all('started_when' in child for child in children):
            step.setdefault('started_when',{'op':'any','args':[child['started_when'] for child in children]})
        return 'production_layout' if all('completion' in child for child in children) else next((r for r in reasons if r not in ('production_live_assignment','production_relative_assignment','production_remaining_assignment')),'production_target_missing')
    if 'production_selector' in params:
        prefix='guide.production.'+step['key']
        for field,keys in [('completion',(prefix+'.assigned',prefix+'.requested',prefix+'.added',prefix+'.remaining_allocated')),('started_when',(prefix+'.started',))]:
            condition=step.get(field,{})
            if owned(condition,prefix+'.'): step.pop(field)
        params.pop('production_selector')
    params.pop('design_reference',None)
    if params.get('mio_name') is not None: return 'production_mio_not_observed'
    context=ResolutionContext(country=country)
    entries=catalog.rows(('equipment',),context)
    query=params.get('equipment_id') if params.get('equipment_id') is not None else params.get('equipment_name')
    if not isinstance(query,str) or not query: return 'production_equipment_unresolved'
    matches=catalog.matches(query,('equipment',),context,explicit=params.get('equipment_id') is not None)
    # Generic labels shared by a family and its first level denote the family.
    roots=[e for e in matches if e.get('is_archetype')]
    if len(roots)==1 and all(e.get('family_id')==roots[0]['id'] for e in matches): matches=roots
    from .reference_designs import selector as reference_selector
    # A known equipment type/family is generic. A library recipe with the same
    # name must not turn it into a named-variant requirement.
    declared=reference_selector(query,catalog,known_variants) if params.get('equipment_id') is None and not matches else []
    if len(declared)>1 and any(row.get('reference') for row in declared):return 'production_equipment_ambiguous'
    if len(declared)==1 and declared[0].get('reference'):
        matches=[]
        params['design_reference']=deepcopy(declared[0]['reference'])
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
        named=catalog.matches(variant,('equipment',),context)
        if len(named)>1: return 'production_variant_ambiguous'
        if named:
            chosen=named[0]; selected=chosen.get('equipment_ids',[chosen['id']])
            if not set(selected)<=set(identifiers): return 'production_variant_incompatible'
            identifiers=selected
            variants=[]  # Catalog equipment names identify types, not native variant names.
    category=params.get('production_category') or (declared[0].get('category') if len(declared)==1 else None)
    if category not in (None,'military','naval'): return 'production_category_invalid'
    if not identifiers: return 'production_equipment_unresolved'
    params['production_selector']={'equipment_ids':sorted(identifiers),'variant_names':sorted(variants),'category':category}
    key='guide.production.'+step['key']
    priority=params.get('production_priority')
    if priority is not None and (priority not in ('first','last') or category!='military' or params.get('quantity_mode')=='additional'):
        return 'production_priority_invalid'
    def complete(condition):
        if priority:condition={'op':'all','args':[condition,_fact(key+'.priority_satisfied','eq',True)]}
        step.setdefault('completion',condition)
    step.setdefault('started_when',_fact(key+'.started','eq',True))
    if params.get('quantity_mode')=='remaining':
        if category!='military' or any(k in params for k in ('requested_factories','assigned_factories','assignment_test')):
            return 'production_target_invalid'
        complete(_fact(key+'.remaining_allocated','eq',True))
        return 'production_remaining_assignment'
    requested='requested_factories' in params
    target=params.get('requested_factories') if requested else params.get('assigned_factories')
    if requested and 'assigned_factories' in params:return 'production_target_invalid'
    if not _integer(target): return 'production_target_missing'
    relative=params.get('quantity_mode')=='additional'
    if relative and target==0: return 'production_target_missing'
    test='gte' if relative else params.get('assignment_test','eq')
    if test not in ('eq','gte'): return 'production_target_invalid'
    complete(_fact(key+('.added' if relative else '.requested' if requested else '.assigned'),test,target))
    return 'production_relative_assignment' if relative else 'production_live_assignment'
