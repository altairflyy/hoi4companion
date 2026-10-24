"""Resolve declared political identities from installed scripts, never run triggers.

English is the reference; local labels and trait names are aliases. A catalog
association proves identity, not availability or completion in a live campaign.
"""
from .clausewitz import Node
from .jomini_parser import parse_file
from .installation import collect_files
from .research_upgrades import exact_matches, normal_name
from .tooltips import localise
from .event_country import country_condition, event_condition, political_condition, combine
import re


def political_catalog(game, mods, labels, reference):
    rows, warnings = [], []
    def names(key):
        english=localise(reference.get(key,key),reference)
        display=localise(labels.get(key,english),labels)
        return {'name':english,'localized_name':display,'aliases':[display] if display!=english else []}
    def read(directory):
        files,issues=collect_files(game,mods,directory,'.txt'); warnings.extend(issues)
        for path in files:
            try: yield path,parse_file(path)
            except (OSError,ValueError,RecursionError) as exc:
                warnings.append(f'Catalogo politica non leggibile ({path.name}): {exc}')
    def traits(node):
        return [value for block in node.blocks('traits') for key,value in block.entries if key=='' and isinstance(value,str)]
    def trait_names(ids):
        return list(dict.fromkeys(text for key in ids for text in [key,names(key)['name'],names(key)['localized_name']]))
    def countries(identifier):
        # A script-defined tag prefix scopes matching, not runtime eligibility.
        hit=re.match(r'^([A-Z0-9]{3})_',identifier)
        return [hit[1]] if hit else []
    metadata={'allowed','available','visible','default','ai_will_do','allowed_civil_war','allowed_to_remove'}
    for path,root in read('common/ideas'):
        for group in root.blocks('ideas'):
            for group_id,definitions in group.entries:
                if not isinstance(definitions,Node): continue
                law=definitions.scalar('law')=='yes'
                for identifier,node in definitions.entries:
                    if not isinstance(node,Node) or identifier in metadata: continue
                    ids=traits(node)
                    rows.append({'kind':'law' if law else 'idea','id':identifier,**names(identifier),
                                 'group':group_id,'traits':ids,'countries':countries(identifier),
                                 'country_condition':political_condition(definitions,node)})
                    rows[-1]['aliases']=list(dict.fromkeys([*rows[-1]['aliases'],*trait_names(ids)]))
    for path,root in read('common/characters'):
        for group in root.blocks('characters'):
            for identifier,node in group.entries:
                if not isinstance(node,Node): continue
                for advisor in node.blocks('advisor'):
                    token=advisor.scalar('idea_token'); slot=advisor.scalar('slot')
                    if not token or not slot: continue
                    ids=traits(advisor); label=names(node.scalar('name',identifier))
                    rows.append({'kind':'advisor','id':token,**label,'character_id':identifier,
                                 'slot':slot,'traits':ids,'countries':countries(identifier),
                                 'country_condition':political_condition(node,advisor),
                                 'aliases':list(dict.fromkeys([*label['aliases'],identifier,*trait_names(ids)]))})
    for path,root in read('events'):
        for kind,event in root.entries:
            if kind not in ('country_event','news_event','state_event','unit_leader_event') or not isinstance(event,Node): continue
            identifier=event.scalar('id'); title=event.scalar('title')
            if not identifier: continue
            rows.append({'kind':'event','id':identifier,**names(title or identifier),
                         'country_condition':event_condition(kind,event)})
            for option in event.blocks('option'):
                key=option.scalar('name')
                if key: rows.append({'kind':'event_option','id':key,'event_id':identifier,**names(key),
                                    'country_condition':combine('all',[country_condition(block) for block in option.blocks('trigger')])})
    categories={}
    for path,root in read('common/decisions/categories'):
        for identifier,node in root.entries:
            if isinstance(node,Node):
                condition=political_condition(node)
                categories[identifier]=combine('any',[categories[identifier],condition]) if identifier in categories else condition
    for path,root in read('common/decisions'):
        if re.search(r'(?:^|/)common/decisions/categories/',path.as_posix(),re.I):continue
        for category,group in root.entries:
            if not isinstance(group,Node): continue
            for identifier,node in group.entries:
                if isinstance(node,Node) and identifier not in metadata:
                    condition=political_condition(group,node)
                    if category in categories:condition=combine('all',[categories[category],condition])
                    rows.append({'kind':'decision','id':identifier,'group':category,**names(identifier),'country_condition':condition})
    # Repeated scripts/roles with the same token must not create fake ambiguity.
    unique={}
    for row in rows:
        key=(row['kind'],row['id'],row.get('slot'),row.get('event_id'))
        if key in unique:
            old=unique[key]
            if 'country_condition' in row:
                old['country_condition']=combine('any',[old['country_condition'],row['country_condition']])
            for field in ('aliases','traits','countries'):
                if field in row: old[field]=list(dict.fromkeys([*old.get(field,[]),*row[field]]))
        else: unique[key]=row
    return list(unique.values()),warnings


def _fact(key,value):
    return {'op':'fact','key':key,'test':'contains','value':value}


def bind_political(step,catalog,country):
    """Compile explicit IDs/names to qualified memberships. No prose strategy."""
    params=step['action'].get('params',{})
    if params.get('category')=='projects':return None
    previous=params.pop('political_binding',{})
    if isinstance(previous,dict):
        for field in ('completion','started_when'):
            if field in previous and step.get(field)==previous[field]:step.pop(field)
    def attach(field,condition):
        if field not in step:
            step[field]=condition
            params.setdefault('political_binding',{})[field]=condition.copy()
    def matches(kinds,identifier,name):
        entries=[row for row in catalog if row['kind'] in kinds and
                 (not country or not row.get('countries') or country in row['countries'])]
        # Explicit IDs must exist verbatim: no fallback to a different name.
        return [row for row in entries if row['id']==identifier] if identifier is not None else exact_matches(name,entries) if isinstance(name,str) and name else []
    def identity(kinds,id_key,name_key):
        found=matches(kinds,params.get(id_key),params.get(name_key))
        return found[0] if len(found)==1 else None, 'politics_identity_ambiguous' if found else 'politics_identity_unresolved'
    if params.get('law_id') is not None or params.get('law_name') is not None:
        item,reason=identity(('law',),'law_id','law_name')
        if not item: return reason
        params['law_id']=item['id']; params['law_group']=item['group']
        attach('completion',_fact('politics.ideas',item['id']))
        return 'politics_live_law'
    if any(params.get(key) is not None for key in ('advisor_id','person_id','person_name','trait_name','role')):
        identifier=params.get('advisor_id'); query=params.get('person_id') or params.get('person_name') or params.get('trait_name') or params.get('role')
        found=matches(('advisor',),identifier,query)
        concerns=matches(('idea',),None,query) if identifier is None and params.get('person_id') is None else []
        concerns=[row for row in concerns if row.get('group') in ('industrial_concern','theorist','political_advisor','army_chief','navy_chief','air_chief','high_command')]
        if not found and len(concerns)==1:
            item=concerns[0];params['idea_id']=item['id'];attach('completion',_fact('politics.ideas',item['id']))
            return 'politics_live_idea'
        slot=params.get('role')
        slots={normal_name(row['slot']):row['slot'] for row in catalog if row['kind']=='advisor'}
        if isinstance(slot,str) and normal_name(slot) in slots: found=[r for r in found if r['slot']==slots[normal_name(slot)]]
        if len(found)!=1: return 'politics_identity_ambiguous' if found else 'politics_identity_unresolved'
        item=found[0]; params['advisor_id']=item['id']; params['person_id']=item['character_id']; params['role']=item['slot']
        attach('completion',_fact('politics.advisors',item['id']))
        return 'politics_live_advisor'
    if any(params.get(key) is not None for key in ('event_id','event_name','event_option','event_option_id')):
        events=matches(('event',),params.get('event_id'),params.get('event_name'))
        if (params.get('event_id') is not None or params.get('event_name') is not None) and len(events)!=1:
            return 'event_identity_ambiguous' if events else 'event_identity_unresolved'
        options=matches(('event_option',),params.get('event_option_id'),params.get('event_option'))
        if events: options=[row for row in options if row['event_id']==events[0]['id']]
        if len(options)==1:
            option=options[0]; params['event_id']=option['event_id']; params['event_option_id']=option['id']
            params['event_option_name']=option.get('localized_name',option['name'])
            attach('started_when',_fact('events.visible_options',option['event_id']+':'+option['id']))
            return 'event_visible_option'
        return 'event_identity_ambiguous' if options else 'event_identity_unresolved'
    if params.get('decision_id') is not None or params.get('decision_name') is not None:
        item,reason=identity(('decision',),'decision_id','decision_name')
        if not item: return reason
        params['decision_id']=item['id']
        # Membership/counters have not been proved to mean active or taken.
        return 'decision_identity_resolved'
    if params.get('idea_id') is not None or params.get('idea_name') is not None:
        item,reason=identity(('idea',),'idea_id','idea_name')
        if not item: return reason
        params['idea_id']=item['id']; attach('completion',_fact('politics.ideas',item['id']))
        return 'politics_live_idea'
    return None
