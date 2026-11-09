"""Source-ordered production revisions; observations never prove past actions."""
import re
from .contracts import evaluate_condition
from .production_progress import selector_for,selector_identity


def annotate_word_revisions(guide):
    """Recover section boundaries from retained Word table source, not filenames."""
    changed=[]
    focuses=[s for rows in guide['tracks'].values() for s in rows if s['action']['kind']=='select_focus']
    sources={s['id']:s for s in guide.get('sources',[]) if s.get('type')=='file' and isinstance(s.get('text'),str)}
    for rows in guide['tracks'].values():
        for row in rows:
            p=row['action'].get('params',{})
            if p.get('category')!='production' or 'production_revision' in p:continue
            for evidence in row.get('evidence',[]):
                source=sources.get(evidence.get('source_id'));quote=evidence.get('quote','')
                if not source or not quote:continue
                text=source['text'];sections=[];previous=[]
                for hit in re.finditer(r'\[Table row\]([\s\S]*?)(?=\[Table row\]|\Z)',text):
                    columns=re.split(r'\[Column (\d+)\]\s*',hit[1])
                    values={columns[i]:columns[i+1] for i in range(1,len(columns)-1,2)}
                    if not all(k in values for k in ('1','2','3')):continue
                    members=[f for f in focuses if any(e.get('source_id')==source['id'] and e.get('quote','').strip()
                        and e['quote'].strip() in values['3'] for e in f.get('evidence',[]))]
                    sections.append((hit,values,members,previous));previous=members or previous
                candidates=[v for v in sections if quote in v[1]['1']]
                if len(candidates)!=1:continue
                hit,values,members,previous=candidates[0]
                targets=p.get('production_targets',[p]);categories={t.get('production_category') for t in targets}
                if len(categories)!=1 or next(iter(categories)) not in ('military','naval'):continue
                instruction='\n'.join(p.get('instructions',[]));offset=quote.find(instruction)
                if offset<0:offset=0
                order=hit.start()+hit[1].find(values['1'])+values['1'].find(quote)+offset
                p['production_revision']={'group':source['id']+':'+next(iter(categories)),'order':order}
                if previous:
                    facts=[{'op':'fact','key':'focus.completed','test':'contains','value':previous[-1]['action']['entity']['id']}]
                    for step in members:
                        for key in ('focus.active','focus.completed'):
                            facts.append({'op':'fact','key':key,'test':'contains','value':step['action']['entity']['id']})
                    # Keep the schema's bounded condition fan-out.
                    while len(facts)>32:facts=[{'op':'any','args':facts[i:i+32]} for i in range(0,len(facts),32)]
                    p['production_section_when']={'op':'any','args':facts}
                changed.append({'key':row['key'],'actions':0,'step_keys':[row['key']],'unverifiable_timing':False})
                break
    return changed


def admission(step,phases,state,results,checks):
    """Check revision gates even when its allocation already matches live counts."""
    p=step['action'].get('params',{})
    guards=[p.get('production_section_when'),phases.get(step.get('phase'),{}).get('when'),step.get('when')]
    values=[evaluate_condition(node,state,checks) for node in guards if node is not None]
    if step.get('skip_when') is not None:
        v=evaluate_condition(step['skip_when'],state,checks)
        values.append(None if v is None else not v)
    if any(v is not True for v in values):
        unknown=not any(v is False for v in values)
        return {'status':'unverifiable' if unknown else 'waiting','reason':'condition_unknown' if unknown else 'condition_false'}
    from .production_milestones import admission as milestone_admission
    milestone=milestone_admission(step,state,checks)
    if milestone:return milestone
    blocked=[k for k in step.get('depends_on',[]) if results[k]['status']!='completed']
    blocked += [k for k in step.get('depends_on_started',[]) if results[k]['selection']['value'] is not True and results[k]['completion']['value'] is not True]
    if blocked:
        unknown=any(results[k]['status']=='unverifiable' for k in blocked)
        return {'status':'unverifiable' if unknown else 'waiting','reason':'dependency_unknown' if unknown else 'dependency_not_completed','blocked_by':blocked}
    return None


def deny(result,decision):
    result.setdefault('original_completion',result['completion'])
    result.setdefault('original_selection',result['selection'])
    result.update(decision,production_admitted=False,completion={'status':'unknown','value':None,'source':'','reason':'revision_not_admitted'},
                  selection={'status':'unknown','value':None,'source':'','reason':'revision_not_admitted'})


def phase_reached(step,phases,state,checks):
    """A known phase boundary is distinct from permission to perform its action.

    Source order, allocations, and missing adoption evidence cannot establish
    this boundary. Explicit dependencies still govern the replacement action.
    """
    p=step['action'].get('params',{})
    guards=[node for node in (p.get('production_section_when'),phases.get(step.get('phase'),{}).get('when'),step.get('when')) if node is not None]
    if not guards or not all(evaluate_condition(node,state,checks) is True for node in guards):return False
    return step.get('skip_when') is None or evaluate_condition(step['skip_when'],state,checks) is False


def apply_revisions(guide,state,results,checks):
    """Mask replaced targets only inside explicitly declared revision groups."""
    if not state['usable']:return
    phases={p['id']:p for p in guide.get('phases',[])}
    protected={k for rows in guide['tracks'].values() for s in rows for k in s.get('depends_on',[])+s.get('depends_on_started',[])}
    admitted=[]
    for rows in guide['tracks'].values():
        for step in rows:
            p=step['action'].get('params',{});revision=p.get('production_revision');result=results[step['key']]
            if not revision or result['reason']=='guide_scope_mismatch':continue
            denied=admission(step,phases,state,results,checks)
            if denied:
                deny(result,denied)
                if not phase_reached(step,phases,state,checks):continue
            targets=p.get('production_targets',[p]);detail=result.get('production',{})
            details=detail.get('layout',[]) if 'production_targets' in p else [detail]
            for i,(target,d) in enumerate(zip(targets,details)):
                selector=selector_for({'action':{'params':target}})
                if selector and target.get('quantity_mode')!='additional':admitted.append((revision['group'],revision['order'],step,i,target,selector,d))
    admitted.sort(key=lambda x:(x[0],x[1]))
    for n,(group,order,step,i,p,selector,detail) in enumerate(admitted):
        if step['key'] in protected:continue
        replacements=[]
        for newer,new_order,other,j,q,selected,_ in admitted[n+1:]:
            if newer!=group or new_order<=order or other['key']==step['key']:continue
            same=selector_identity(selected)==selector_identity(selector)
            capacity=p.get('quantity_mode')=='remaining' and q.get('quantity_mode')=='remaining' and selector['category']==selected['category']=='military'
            if same or capacity:replacements.append(other['key'])
        if replacements:detail['superseded_by']=list(dict.fromkeys(replacements))
    for rows in guide['tracks'].values():
        for step in rows:
            r=results[step['key']];p=step['action'].get('params',{});d=r.get('production',{})
            details=d.get('layout',[]) if 'production_targets' in p else [d]
            replaced=[x for x in details if x.get('superseded_by')]
            if not replaced:continue
            r['superseded_by']=list(dict.fromkeys(k for x in replaced for k in x['superseded_by']))
            remaining=[(i,x) for i,x in enumerate(details) if not x.get('superseded_by')]
            completion=step.get('completion',{})
            clauses=completion.get('args',[]) if 'production_targets' in p and completion.get('op')=='all' else [completion]
            values=[evaluate_condition(clauses[i],state,checks) for i,_ in remaining] if len(clauses)==len(details) else [None]
            r['original_completion']=r['completion']
            # Replacement is not evidence that the old configuration was ever fulfilled.
            # A partially replaced but blocked action must retain its gates and
            # unknown start/completion; replacement must not release dependencies.
            if remaining and r.get('production_admitted') is False:continue
            if not remaining or all(v is True for v in values):
                r.update(status='not_applicable',reason='production_superseded',completion={'status':'unknown','value':None,'source':'','reason':'requirements_replaced'})
            elif any(v is False for v in values):
                r.update(status='active' if r['selection']['value'] is True else 'ready',reason='production_partially_superseded',completion={'status':'calculated','value':False,'source':'effective_production_goal'})
            else:r.update(status='unverifiable',reason='production_evidence_unknown')
