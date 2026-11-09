"""Pure bindings and qualified memberships, never inferred diplomatic outcomes."""
import hashlib
import json
import re
from .contracts import fact
from .research_upgrades import normal_name

MISSIONS=('no_mission','build_intel_network','quiet_network','counter_intelligence','root_out_resistance','boost_ideology','control_trade','diplomatic_pressure','propaganda')
STATES=('captured','cooldown','disbanding','active','operation','killed')
FACTS=('diplomacy.countries','diplomacy.improving_relations','diplomacy.non_aggression_pacts','intelligence.countries','intelligence.active_missions')
from pathlib import Path
TASKS={key:tuple(values) for key,values in json.loads(Path(__file__).with_name('foreign-tasks-v1.json').read_text(encoding='utf8'))['tasks'].items()}
ALIASES={normal_name(name):key for key,names in TASKS.items() for name in (key,*names)}


def valid_tags(value):
    return (isinstance(value,list) and len(value)<=8192 and all(isinstance(x,str) and re.fullmatch('[A-Z0-9]{3}',x) for x in value) and len(set(value))==len(value))


def valid_membership(key,value):
    if key not in FACTS:return True
    if key!='intelligence.active_missions':return valid_tags(value)
    return (isinstance(value,list) and len(value)<=4096
        and all(isinstance(x,str) and ':' in x and x.split(':')[0] in MISSIONS[1:] and re.fullmatch('[A-Z0-9]{3}',x.split(':',1)[1]) for x in value)
        and len(set(value))==len(value))


def facts(overview):
    result={k:fact(reason='Foreign interaction not qualified') for k in FACTS}
    dip=overview.get('diplomacy',{});dip=dip if isinstance(dip,dict) else {}
    d=dip.get('interactions',{});d=d if isinstance(d,dict) else {};tags=d.get('countries')
    if (dip.get('status')=='observed' and d.get('status')=='observed' and valid_tags(tags)
        and all(valid_tags(d.get(k)) and set(d[k])<=set(tags) for k in ('improving_relations','non_aggression_pacts'))):
        for key in ('countries','improving_relations','non_aggression_pacts'):
            result['diplomacy.'+key]=fact(sorted(d[key]),'calculated','foreign_interaction_adapter')
    intel=overview.get('intelligence',{});intel=intel if isinstance(intel,dict) else {}
    m=intel.get('missions',{});m=m if isinstance(m,dict) else {};tags=m.get('countries');rows=m.get('items')
    if (intel.get('status')=='observed' and m.get('status')=='observed' and valid_tags(tags)
        and isinstance(rows,list) and len(rows)<=4096
        and all(isinstance(r,dict) and isinstance(r.get('id'),str) and 0<len(r['id'])<=160 and r.get('mission_id') in MISSIONS and r.get('state') in STATES
                and (r.get('target_country') in tags if r.get('mission_id')!='no_mission' else r.get('target_country') is None) for r in rows)
        and len({r['id'] for r in rows})==len(rows)):
        active={r['mission_id']+':'+r['target_country'] for r in rows if r['state']=='active' and r['mission_id']!='no_mission'}
        result['intelligence.countries']=fact(sorted(tags),'calculated','foreign_interaction_adapter')
        result['intelligence.active_missions']=fact(sorted(active),'calculated','foreign_interaction_adapter')
    return result


def _fact(key,test,value):return {'op':'fact','key':key,'test':test,'value':value}


def predicate(params):
    op=params.get('operation_id');target=params.get('target_country')
    if not isinstance(target,str) or not re.fullmatch('[A-Z0-9]{3}',target):return None
    if op=='assign_operative_mission' and params.get('mission_id') in MISSIONS[1:]:
        return {'op':'all','args':[_fact('intelligence.countries','contains',target),_fact('intelligence.active_missions','contains',params['mission_id']+':'+target)]}
    if op in ('improve_relations','non_aggression_pact','stop_improving_relations'):
        member=_fact('diplomacy.non_aggression_pacts' if op=='non_aggression_pact' else 'diplomacy.improving_relations','contains',target)
        if op=='stop_improving_relations':member={'op':'not','arg':member}
        return {'op':'all','args':[_fact('diplomacy.countries','contains',target),member]}
    return None


def token(step,node):
    return 'foreign:'+hashlib.sha256(json.dumps([step['key'],node],sort_keys=True,separators=(',',':')).encode()).hexdigest()


def bind(step,catalog):
    from .backend_core import _request,_publish_in_place
    from .resolution import catalog_index
    index=catalog_index(catalog)
    result=_request(0,'import_bind_foreign',step=step,**index.native_import_args())
    _publish_in_place(step,result['step'])
    return result['bound']


def definitions(guide):
    result={}
    for rows in guide['tracks'].values():
        for step in rows:
            node=predicate(step['action'].get('params',{}))
            if node is None or step.get('tracking')=='reminder':continue
            identifier=token(step,node)
            if step.get('completion')=={'op':'any','args':[node,_fact('guide.campaign.milestones','contains',identifier)]}:
                result[identifier]={'step':step['key'],'token':identifier,'condition':node,'foreign_interaction':True}
    return result


def commands(text,catalog):
    """Whole clauses only; omitted target can refer to the previous local clause."""
    if not isinstance(text,str) or len(text)>4096:return None
    from .resolution import catalog_index
    catalog=catalog_index(catalog)
    previous=None;result=[];prior_group=[]
    for clause in text.split(','):
        clause=clause.strip();body=None;target=None
        cost=re.search(r'\s*\((\d+)\s*PP\)\s*$',clause,re.I)
        if cost:clause=clause[:cost.start()].strip()
        for hit in re.finditer(r'\s+(?:(?:with|in|against|on|verso|con)\s+)?',clause,re.I):
            query=clause[hit.end():]
            candidate=None if normal_name(query) in ALIASES else catalog.resolve_country(query)
            if candidate:body,target=clause[:hit.start()].strip(),candidate;break
        if body is None:body=clause;target=previous
        if target is None:return None
        group=[]
        for label in re.split(r'\s*[+&]\s*',body):
            key=ALIASES.get(normal_name(label))
            if key is None:return None
            params={'category':'politics' if key not in MISSIONS else 'projects','operation_id':key,'target_country':target}
            if key in MISSIONS:params.update(operation_id='assign_operative_mission',mission_id=key)
            if cost:params['source_group_cost']=cost[0].strip()
            group.append(len(result));result.append({'text':label.strip()+' '+target,'params':params,'after_keys':prior_group.copy(),'after_previous':bool(prior_group)})
        previous=target;prior_group=group
    return result or None


def prefix_commands(text,catalog):
    """A complete supported comma-prefix may precede an untouched method note."""
    full=commands(text,catalog)
    if full:return full,''
    cuts=[m.start() for m in re.finditer(',',text)]
    for cut in reversed(cuts):
        parsed=commands(text[:cut].strip(),catalog)
        if parsed:return parsed,text[cut+1:].strip()
    return None,text
