"""Installed doctrine identities and contextual guide commands; no native reads."""
import re
from .clausewitz import Node
from .installation import collect_files
from .jomini_parser import parse_file
from .tooltips import localise
from .resolution import ResolutionContext
from .research_upgrades import guide_language,normal_name
from .contracts import EXPERIENCE_CURRENCIES


def doctrine_catalog(game,mods,labels,reference):
    rows=[];files,issues=collect_files(game,mods,'common/doctrines','.txt')
    for path in files:
        if not any(p in ('grand_doctrines','subdoctrines') for p in path.parts):continue
        try:
            for identifier,node in parse_file(path).entries:
                if not isinstance(node,Node) or not node.scalar('name'):continue
                currency={'army':'army_experience','navy':'navy_experience','air':'air_experience'}.get(node.scalar('xp_type'))
                domain={'army_experience':'land','navy_experience':'naval','air_experience':'air'}.get(currency)
                if domain is None:continue
                key=node.scalar('name');name=localise(reference.get(key,key),reference)
                display=localise(labels.get(key,name),labels)
                rows.append({'kind':'doctrine','id':identifier,'name':name,'localized_name':display,
                    'aliases':[display] if display!=name else [],'domain':domain,
                    'track':node.scalar('track','grand'),'currency':currency})
        except (OSError,ValueError,RecursionError) as exc:issues.append(f'Doctrine catalog unreadable ({path.name}): {exc}')
    return rows,issues


def doctrine_command(text,index,country):
    value=re.sub(r'^ASAP(?:\s*:\s*|\s+)','',text.strip(),flags=re.I)
    prefix=None
    # Adjacent complete commands separated by the author's ASAP marker are
    # independent source actions. Never discard a leading unknown condition.
    adjacent=re.fullmatch(r'(.+?)\s+ASAP:\s*(.+)',value,re.I)
    if adjacent:
        from .informal_targets import named_command
        prefix=named_command(adjacent[1],index,country)
        if not prefix or prefix.get('timing'):return None
        prefix={**prefix,'text':adjacent[1]};value=adjacent[2]
    hit=re.match(r'(.+?)\s+(Land|Air|Naval|Navy|Sea)\s+Doc(?:trine)?\b',value,re.I)
    if not hit:return None
    tail=value[hit.end():].strip();tactic=False;notes=[];cost=None;unit=None
    m=re.match(r'^[+&]\s*Tactic\b',tail,re.I)
    if m:tactic=True;tail=tail[m.end():].strip();notes.append('Preferred Tactic (not specified)')
    tail=re.sub(r'^ASAP\b\s*','',tail,flags=re.I)
    cost_match=re.match(r'^\(\s*(\d{1,6})\s*(AE|NE|AFE)(?:\s*,\s*discounted)?\s*\)',tail,re.I)
    if cost_match:
        cost=int(cost_match[1]);unit=cost_match[2].upper();tail=tail[cost_match.end():].strip()
    tail=re.sub(r'^ASAP\b\s*','',tail,flags=re.I)
    # Explicit build-reference notes are descriptive. Other parenthesized
    # qualifiers (especially conditions) cannot be silently detached.
    build=re.match(r'^\(([^()]+\s+BUILD)\)',tail,re.I)
    if build:notes.append(build[0]);tail=tail[build.end():].strip()
    if tail:
        remainder=re.fullmatch(r'[,;+&]\s*(.+)',tail)
        if not remainder or re.search(r'\b(?:if|when|unless|until|before|after|instead|or)\b',remainder[1],re.I):return None
        notes.append(remainder[1])
    domain={'land':'land','air':'air','naval':'naval','navy':'naval','sea':'naval'}[hit[2].lower()]
    entries=guide_language().get('abbreviations',{}).get(domain+'_doctrines',{})
    query=next((name for alias,name in entries.items() if normal_name(alias)==normal_name(hit[1])),hit[1].strip())
    matches=index.matches(query,('doctrine',),ResolutionContext(country=country),domain=domain)
    if len(matches)!=1:return None
    row=matches[0];currency=row['currency']
    if unit in ('NE','AFE') and currency!={'NE':'navy_experience','AFE':'air_experience'}[unit]:return None
    params={'category':'army','doctrine_id':row['id'],'doctrine_name':row['name'],
            'doctrine_domain':domain,'doctrine_track':row['track'],'vocabulary_binding':{'input':hit[1].strip(),'expanded':query,'domain':domain+'_doctrines'}}
    policies={}
    # A combined doctrine+tactic cost cannot be assigned to just one action.
    if cost is not None and not tactic and not notes:policies['resource_costs']={EXPERIENCE_CURRENCIES[currency]:cost}
    # A cost before an explicit comma belongs to the doctrine, even when the
    # following method names other actions; combined '+' costs remain unknown.
    elif cost is not None and not tactic and cost_match:policies['resource_costs']={EXPERIENCE_CURRENCIES[currency]:cost}
    return {'params':params,'policies':policies,'remainder':'\n'.join(notes) if notes else None,'prefix':prefix,
            'text':value[:hit.end()]}


def bind_doctrine(step,index,country):
    p=step['action'].get('params',{});query=p.get('doctrine_id') or p.get('doctrine_name')
    matches=index.matches(query,('doctrine',),ResolutionContext(country=country),explicit=bool(p.get('doctrine_id')))
    if len(matches)!=1:return 'doctrine_identity_unresolved'
    row=matches[0];p.update(doctrine_id=row['id'],doctrine_name=row['name'],doctrine_domain=row['domain'],doctrine_track=row['track'])
    step.setdefault('completion',{'op':'fact','key':'doctrines.selected','test':'contains','value':row['id']})
    return 'doctrine_selection_unobserved'
