"""Resolve complete informal commands through installed, country-scoped data."""
from copy import deepcopy
import re

from .resolution import ResolutionContext
from .research_upgrades import guide_language
from .political_import import bind_political
from .contracts import EXPERIENCE_CURRENCIES, MILITARY_SPIRIT_GROUPS, SPIRIT_CURRENCIES


_ROLES={'industrial_concern','theorist','political_advisor','army_chief','navy_chief','air_chief','high_command'}
_NAMED=re.compile(r'([^:\n&]+?)(?:\s*\(\s*(\d{1,6})\s*(PP|AE|NE|AFE)\s*\))?'
                  r'(?:\s*\(\s*((?:before|after|when|if|on)\s+[^()]+)\s*\))?\s*',re.I)


def named_command(text,index,country):
    if not country:return None
    text=re.sub(r'^ASAP(?:\s*:\s*|\s+)','',text,flags=re.I).strip()
    match=_NAMED.fullmatch(text)
    if not match:return None
    query=match[1].strip()
    found=[entry for kind in ('advisor','idea','decision')
           for entry in index.matches(query,(kind,),ResolutionContext(country=country))]
    found=[e for e in found if e['kind']!='idea' or e.get('group') in _ROLES|MILITARY_SPIRIT_GROUPS]
    if len(found)!=1:return None
    entry=found[0]
    if entry['kind']=='decision' and re.search(r'\[(?:FROM|TARGET)(?:\.|\])',entry['name'],re.I):return None
    currency=(match[3] or '').upper()
    count=int(match[2]) if match[2] is not None else None
    params={'category':'characters' if entry['kind']=='advisor' else 'politics'}
    extra={}
    if entry['kind']=='advisor':params.update(advisor_id=entry['id'],person_name=entry['name'],role=entry['slot'])
    elif entry['kind']=='idea':params.update(idea_id=entry['id'],idea_group=entry['group'])
    else:
        params.update(decision_id=entry['id'],decision_name=entry['name'],decision_group=entry.get('group'))
        # This is the required evidence, not a claim that the native reader
        # exposes decision execution. Listing/disappearance is insufficient.
        extra['completion']={'op':'fact','key':'decisions.executed','test':'contains','value':entry['id']}
    if currency=='PP':extra['political_power_cost']=count
    elif currency:
        if entry['kind']!='idea' or entry.get('group') not in MILITARY_SPIRIT_GROUPS:return None
        # Informal AE is overloaded between army and air experience. Once the
        # selected spirit is unique its native group supplies the currency.
        key=SPIRIT_CURRENCIES[entry['group']] if currency=='AE' else {'NE':'navy_experience','AFE':'air_experience'}[currency]
        if SPIRIT_CURRENCIES[entry['group']]!=key:return None
        params['cost_currency']=key
        extra['resource_costs']={EXPERIENCE_CURRENCIES[key]:count}
    probe={'action':{'kind':'manual','params':params}}
    reason=bind_political(probe,index,country)
    if reason not in ('politics_live_advisor','politics_live_idea','decision_identity_resolved'):return None
    return {'params':probe['action']['params'],'policies':extra,'timing':match[4]}


def production_command(text,index,country,known_variants=None):
    match=re.fullmatch(r'\s*(MIL|NAV|Production)\s*:\s*(.+)',text,re.I)
    if not match:
        # A complete equipment allocation is also meaningful without MIL:.
        # ID binding below still rejects prose, unknown products and quantities.
        body=re.sub(r'^design\s*(?:&|and)\s*produce\s+','',text.strip(),flags=re.I)
        if ':' not in body:return None
        body=re.sub(r'\s*\(\s*\d+\s*(?:AE|NE|AFE|PP)\s*\)(?=\s*:)', '',body,flags=re.I)
        match=re.fullmatch(r'(MIL):\s*(.+)','MIL: '+body,re.I)
    from .operation_import import _bind_production
    category={'mil':'military','nav':'naval'}.get(match[1].casefold())
    aliases={k.casefold():v for k,v in guide_language()['abbreviations']['production'].items() if k not in ('MIL','NAV')}
    # Comma-separated numeric groups are explicit shorthand. Missing separators
    # and conjunctions retain their original reminder instead of guessing.
    if re.search(r'\s&\s',match[2]):return None
    parts=[];notes=[];priorities={};original_parts=[]
    for part in (p.strip() for p in match[2].split(',') if p.strip()):
        note=re.fullmatch(r'(.*?)\s*(\((?:add\s+)?MIOs?\))',part,re.I)
        if note:parts.append(note[1].strip());notes.append(note[2])
        else:parts.append(part)
        original_parts.append(parts[-1])
        priority=re.fullmatch(r'(.*?)\s*\((both\s+)?(top|highest|lowest)\s+(?:prio|priority)\)',parts[-1],re.I)
        if priority:
            parts[-1]=priority[1].strip();mode='last' if priority[3].lower()=='lowest' else 'first'
            priorities[len(parts)-1]=mode
            if priority[2] and len(parts)>1:priorities[len(parts)-2]=mode
    if any(re.fullmatch(r'\d+',part) for part in parts):return None
    item_pattern=r'([^:\n]+?)\s*(?::\s*|\s+to\s+|\s+)(\d{1,6}(?:\s*(?:->|→)\s*\d{1,6})?|ALL|rest)\s*'
    parsed=[]
    all_parts=[i for i,part in enumerate(parts) if (m:=re.fullmatch(item_pattern,part,re.I)) and m[2].lower() in ('all','rest')]
    # A trailing numeric allocation qualifies the preceding comma-separated
    # products up to the previous explicit quantity. ALL never distributes.
    shared={};pending=[]
    for i,part in enumerate(parts):
        entry=re.fullmatch(item_pattern,part,re.I)
        if not entry:pending.append(i);continue
        if entry[2].isdigit():
            for previous in pending:shared[previous]=entry[2]
        pending=[]
    for i,part in enumerate(parts):
        entry=re.fullmatch(item_pattern,part+':'+shared[i] if i in shared else part,re.I)
        if not entry:
            parsed.append((part,None));continue
        name=aliases.get(entry[1].strip().casefold(),entry[1].strip())
        remaining=entry[2].lower() in ('all','rest')
        # The final military product receives the remaining capacity. Multiple
        # ALL targets, naval requests and a following product do not define it.
        if remaining and (category=='naval' or all_parts!=[len(parts)-1] or i!=len(parts)-1):
            parsed.append((part,None));continue
        params={'equipment_name':name,'category':'production'}
        if remaining:params.update(quantity_mode='remaining',production_category='military')
        else:
            bounds=re.split(r'\s*(?:->|→)\s*',entry[2])
            if len(bounds)==2 and int(bounds[0])>int(bounds[1]):
                parsed.append((part,None));continue
            params['requested_factories']=int(bounds[-1])
        if category:params['production_category']=category
        if i in priorities:
            params.update(production_priority=priorities[i],production_category='military')
        probe={'key':'probe','action':{'kind':'manual','params':params}}
        if _bind_production(probe,index,country,known_variants) not in ('production_live_assignment','production_remaining_assignment'):
            parsed.append((part,None));continue
        parsed.append((part,deepcopy(probe['action']['params'])))
    for priority in ('first','last'):
        group=[i for i,mode in priorities.items() if mode==priority]
        if any(parsed[i][1] is None for i in group):
            for i in group:parsed[i]=(parsed[i][0],None)
    signatures=[tuple(p['production_selector']['equipment_ids']) for _,p in parsed if p]
    repeated={a for i,a in enumerate(signatures) for j,b in enumerate(signatures) if i!=j and set(a)&set(b)}
    targets=[];remaining=[]
    for i,(part,params) in enumerate(parsed):
        if not params or tuple(params['production_selector']['equipment_ids']) in repeated:remaining.append(original_parts[i])
        else:targets.append(params)
    remaining.extend(notes)
    if not targets or len(targets)>64:return None
    return {'params':{'category':'production','production_targets':targets},
            'remainder':match[1]+': '+', '.join(remaining) if remaining else None}
