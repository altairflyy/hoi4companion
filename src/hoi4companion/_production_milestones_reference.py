"""Source-ordered adoption milestones, separate from dates and allocations."""
from copy import deepcopy
import hashlib
import json
import re
from .contracts import evaluate_condition

HISTORY='guide.campaign.milestones'
MEMBERSHIPS={'politics.ideas','politics.advisors','research.completed','focus.completed','projects.completed','doctrines.selected'}


def token_for(key,node):
    return json.dumps([key,node['key'],node['value']],ensure_ascii=False,separators=(',',':'))


def generated_signature(values):
    return hashlib.sha256(json.dumps(values,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()).hexdigest()


def eligible(step):
    node=step.get('completion',{})
    return (step.get('tracking')!='reminder' and node.get('op')=='fact'
        and node.get('key') in MEMBERSHIPS and node.get('test')=='contains'
        and isinstance(node.get('value'),str) and bool(node['value'])
        and not any(step.get(k) for k in ('when','skip_when','depends_on','depends_on_started')))


def _validate_one(value,rows,key):
    if not isinstance(value,dict) or set(value)!={'step','token','condition'}:raise ValueError('Invalid production milestone')
    anchor=next((r for r in rows if r['key']==value['step']),None)
    if (anchor is None or anchor['key']==key or not eligible(anchor) or anchor['completion']!=value['condition']
            or not isinstance(value['token'],str) or len(value['token'])>2048
            or value['token']!=token_for(anchor['key'],anchor['completion'])):
        raise ValueError('Invalid production milestone reference')


def validate(values,rows,key):
    if not isinstance(values,list) or not 1<=len(values)<=64:raise ValueError('Invalid production milestones')
    for value in values:_validate_one(value,rows,key)
    if len({v['step'] for v in values})!=len(values):raise ValueError('Duplicate production milestone')


def definitions(guide):
    from .foreign_interactions import definitions as foreign_definitions
    return {**{p['token']:p for rows in guide['tracks'].values() for row in rows
            for p in row['action'].get('params',{}).get('production_milestones',[])},**foreign_definitions(guide)}


def annotate(guide):
    """Latest mandatory adoption per domain in the ordered Word action column.

    Parallel research/focus columns are never treated as sequential action
    prerequisites. Explicit dependencies and authored milestone annotations win.
    """
    if guide.get('dependency_policy')!='explicit':return []
    rows=[r for lane in guide['tracks'].values() for r in lane];changed=[]
    for source in guide.get('sources',[]):
        if source.get('type')!='file':continue
        text=source.get('text','');columns=[]
        for hit in re.finditer(r'\[Table row\]([\s\S]*?)(?=\[Table row\]|\Z)',text):
            pieces=re.split(r'\[Column (\d+)\]\s*',hit[1])
            values={pieces[i]:pieces[i+1] for i in range(1,len(pieces)-1,2)}
            if all(k in values for k in ('1','2','3')):
                columns.append((hit.start(),values['1']))
        located=[]
        for row in rows:
            p=row['action'].get('params',{});instructions=p.get('instructions',[])
            if len(instructions)!=1:continue
            instruction=instructions[0].strip()
            if not instruction:continue
            positions=set()
            for e in row.get('evidence',[]):
                quote=e.get('quote','')
                if e.get('source_id')!=source['id'] or not quote or quote.count(instruction)!=1:continue
                for start,column in columns:
                    if column.count(quote)==1:
                        positions.add(start+column.index(quote)+quote.index(instruction))
            if len(positions)==1:located.append((positions.pop(),row))
        located.sort(key=lambda pair:pair[0]);anchors={}
        for position,row in located:
            p=row['action'].get('params',{})
            if eligible(row):
                bucket=(row['completion']['key'],p.get('law_group') or p.get('role') or p.get('idea_group')
                        or (p['doctrine_domain']+':'+p['doctrine_track'] if p.get('doctrine_domain') and p.get('doctrine_track') else row['action']['kind']))
                anchors[bucket]=row;continue
            if (p.get('category')!='production' or row.get('tracking')=='reminder' or not anchors
                    or 'depends_on' in row or 'depends_on_started' in row):continue
            previous=p.get('production_milestones')
            if previous is not None and p.get('production_milestones_generated')!=generated_signature(previous):continue
            # Position ties are ambiguous, not an ordering declaration.
            if sum(1 for pos,_ in located if pos==position)!=1:continue
            milestones=[]
            for anchor in anchors.values():
                condition=deepcopy(anchor['completion'])
                token=token_for(anchor['key'],condition)
                milestones.append({'step':anchor['key'],'token':token,'condition':condition})
            if len(milestones)>64:continue
            if previous==milestones:continue
            p['production_milestones']=milestones
            p['production_milestones_generated']=generated_signature(milestones)
            changed.append({'key':row['key'],'actions':0,'step_keys':[row['key']],'unverifiable_timing':False})
    return changed


def admission(step,state,checks):
    milestones=step['action'].get('params',{}).get('production_milestones',[])
    blocked=[];values=[]
    for p in milestones:
        current=evaluate_condition(p['condition'],state,checks)
        historical=evaluate_condition({'op':'fact','key':HISTORY,'test':'contains','value':p['token']},state,checks)
        if current is True or historical is True:continue
        blocked.append(p['step']);values.append(current)
    if not blocked:return None
    known_false=any(v is False for v in values)
    return {'status':'waiting' if known_false else 'unverifiable',
            'reason':'milestone_not_completed' if known_false else 'milestone_unknown','blocked_by':blocked}
