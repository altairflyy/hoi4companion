"""Declared alternative research branches; no strategy, clicks or game I/O."""
from copy import deepcopy
import re


def validate_choice(step):
    choice=step.get('research_choice')
    if choice is None:return
    if (step['action']['kind']!='research' or not isinstance(choice,dict)
            or set(choice)-{'group','query','branches','level','mode'} or not {'group','query','branches'}<=set(choice)
            or choice.get('mode','persistent') not in ('persistent','once')
            or choice.get('mode')=='once' and 'level' in choice
            or any(not isinstance(choice[k],str) or not 0<len(choice[k])<=512 for k in ('group','query'))
            or not isinstance(choice['branches'],dict) or not 2<=len(choice['branches'])<=8
            or 'relative_research' in step):
        raise ValueError('Scelta ramo ricerca non valida')
    ids=[]
    for key,chain in choice['branches'].items():
        if (not isinstance(key,str) or not 0<len(key)<=160 or not isinstance(chain,list) or not 1<=len(chain)<=128
                or any(not isinstance(x,str) or not 0<len(x)<=512 for x in chain)):
            raise ValueError('Ramo alternativo non valido')
        ids.extend(chain)
    if len(ids)!=len(set(ids)) or 'level' in choice and (type(choice['level']) is not int or not 1<=choice['level']<=128):
        raise ValueError('Livello o identità alternative non validi')
    if 'level' in choice and any(choice['level']>len(chain) for chain in choice['branches'].values()):
        raise ValueError('Livello alternativo non presente')


def compile_choices(guide,by_id,resolver=None):
    from ._research_upgrades_reference import guide_language,normal_name,relative_chain,expanded_research_name
    definitions=guide_language().get('research_branch_choices',{})
    for rows in guide['tracks'].values():
        declared={}
        for step in rows:
            if step['action']['kind']!='research':continue
            query=step.get('research_choice',{}).get('query',step.get('relative_research',{}).get('query',step['action']['entity']['id']))
            if '_' in query:continue
            match=re.fullmatch(r'\s*(.+?)(?:\s+(UP|UPGRADES|\d+|III|II|I|IV|V))?\s*',query,re.I)
            if not match:continue
            stem,suffix=match.groups();is_up=bool(suffix and suffix.casefold() in ('up','upgrades'))
            explicit=next(((key,data) for key,data in definitions.items()
                           if normal_name(stem) in {normal_name(x) for x in data['aliases']}),None)
            inherited=next(((key,data) for key,data in declared.items() if normal_name(expanded_research_name(stem))
                            in {normal_name(x) for x in data['families'].values()}),None)
            selected=explicit or inherited
            if not selected:
                if '/' not in query:continue
                from .resolution import catalog_index
                resolver=resolver or catalog_index(by_id.values())
                branches={}
                parts=[x.strip() for x in query.split('/')]
                if not 2<=len(parts)<=8 or any(not part for part in parts):continue
                for i,part in enumerate(parts):
                    up=re.fullmatch(r'(.+?)\s+(?:UP|UPGRADES)',part,re.I)
                    matches=resolver.matches(part,('technology',)) if not up else []
                    chain=relative_chain(up[1],by_id) if up else [matches[0]['id']] if len(matches)==1 else None
                    if not chain:break
                    branches['option_'+str(i+1)]=chain
                else:
                    if len({x for chain in branches.values() for x in chain})!=sum(map(len,branches.values())):continue
                    step['research_choice']={'group':'step:'+step['key'],'query':query,'branches':branches,'mode':'once'}
                    step.pop('relative_research',None);step.pop('research_upgrade',None)
                    step['action']['entity']['id']=next(iter(branches.values()))[0]
                continue
            key,data=selected
            branches={branch:relative_chain(family,by_id) for branch,family in data['families'].items()}
            if any(not chain for chain in branches.values()):continue
            choice={'group':key,'query':query,'branches':branches}
            if suffix and not is_up:
                choice['level']=int(suffix) if suffix.isdecimal() else {'i':1,'ii':2,'iii':3,'iv':4,'v':5}[suffix.casefold()]
                if any(choice['level']>len(chain) for chain in branches.values()):continue
            step['research_choice']=choice
            step.pop('relative_research',None);step.pop('research_upgrade',None)
            step['action']['entity']['id']=next(iter(branches.values()))[choice.get('level',1)-1]
            declared[key]=data


def context(guide,state,bindings,offered=None):
    """Qualified observed research and stored targets jointly identify a branch."""
    rows=[s for key in sorted(guide['tracks']) for s in guide['tracks'][key]]
    choices=[s for s in rows if s.get('research_choice')]
    result={s['key']:{'options':[],'branch':None,'status':'unknown'} for s in choices}
    if not choices or not state.get('usable'):return result
    values=[]
    for key in ('research.completed','research.active'):
        fact=state.get('facts',{}).get(key,{})
        if fact.get('status')!='observed' or not fact.get('source') or not isinstance(fact.get('value'),list):return result
        values.append(set(fact['value']))
    completed,active=values;groups={}
    offered=offered if isinstance(offered,dict) else {}
    for step in choices:
        validate_choice(step);choice=step['research_choice'];group=choice['group']
        if group not in groups:groups[group]=choice['branches']
        elif groups[group]!=choice['branches']:return result
    selected={}
    for group,branches in groups.items():
        used={bindings.get(s['key']) for s in choices if s['research_choice']['group']==group}
        seen={key for key,chain in branches.items() if (completed|active|used)&set(chain)}
        selected[group]=next(iter(seen)) if len(seen)==1 else False if seen else None
    previous={}
    for step in choices:
        choice=step['research_choice'];group=choice['group'];branch=selected[group]
        info=result[step['key']];info['branch']=branch if branch else None
        bound=bindings.get(step['key'])
        if bound in {x for chain in choice['branches'].values() for x in chain}:
            info.update(status='bound',options=[bound]);previous.setdefault(group,[]).append(step);continue
        if choice.get('mode')=='once':
            saved=offered.get(step['key'])
            valid_saved=isinstance(saved,list) and 1<=len(saved)<=len(choice['branches']) and all(
                isinstance(x,str) and any(x in chain for chain in choice['branches'].values()) for x in saved) and len(saved)==len(set(saved)) and all(
                    sum(x in chain for x in saved)<=1 for chain in choice['branches'].values())
            options=[]
            earlier=rows[:rows.index(step)]
            for _,chain in sorted(choice['branches'].items()):
                frozen=next((x for x in saved if x in chain),None) if valid_saved else None
                blocked=any((bindings.get(p['key'],p['action'].get('entity',{}).get('id')) in chain
                             and bindings.get(p['key'],p['action'].get('entity',{}).get('id')) not in completed)
                            or (p.get('relative_research',{}).get('chain')==chain or chain in p.get('research_choice',{}).get('branches',{}).values())
                               and p['key'] not in bindings for p in earlier if p['action']['kind']=='research') if len(chain)>1 else False
                target=frozen or (None if blocked else chain[0] if len(chain)==1 else next((x for x in chain if x not in completed),None))
                if target:options.append(target)
            # Older completed levels of an UP are not a choice in this step.
            chosen=[x for x in options if x in active or x in completed and (valid_saved or any(chain==[x] for chain in choice['branches'].values()))]
            if len(chosen)==1:
                info.update(branch=next(k for k,chain in choice['branches'].items() if chosen[0] in chain),status='selected',options=chosen)
            elif chosen:info.update(status='conflict',options=[])
            else:info.update(branch=None,status='open',options=options)
            continue
        if branch is False:info['status']='conflict'
        elif any(bindings.get(s['key']) not in completed for s in previous.get(group,[])):info['status']='waiting'
        else:
            for key in sorted(choice['branches']):
                chain=choice['branches'][key]
                if branch and key!=branch:continue
                target=chain[choice['level']-1] if 'level' in choice else next((x for x in chain if x not in completed),None)
                if target:info['options'].append(target)
            info['status']='selected' if branch else 'open'
        previous.setdefault(group,[]).append(step)
    return result


def project(guide,bindings):
    result=deepcopy(guide)
    for rows in result['tracks'].values():
        for step in rows:
            if not step.get('research_choice'):continue
            validate_choice(step)
            ids={x for chain in step['research_choice']['branches'].values() for x in chain}
            selected=bindings.get(step['key'])
            if selected not in ids:selected='unbound-choice:'+step['key'];step['catalog_match']=False
            else:step.pop('catalog_match',None)
            step['action']['entity']['id']=selected;step['target']=selected
    return result
