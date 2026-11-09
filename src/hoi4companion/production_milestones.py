"""Source-ordered adoption milestones, separate from dates and allocations."""
import hashlib
import json
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
    from .backend_core import _request,_publish_in_place
    result=_request(0,'import_annotate_milestones',guide=guide)
    _publish_in_place(guide,result['guide'])
    return result['changes']


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
