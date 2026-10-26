"""Presentation of declared costs/requirements; no inferred prices or strategy."""
from .contracts import evaluate_condition, is_json_number


def action_domain(step):
    params=step['action'].get('params',{})
    if step.get('tracking')=='reminder':return None
    if step['action']['kind']=='special_project':return 'project'
    if params.get('commander_id') or params.get('role') in ('corps_commander','navy_leader'):return 'appointment'
    if 'political_power_cost' in step or any(params.get(k) for k in ('law_id','law_name','idea_id','idea_name','advisor_id','person_id','person_name','trait_name','role')):return 'political'
    return None


def readiness(step,state,checks):
    domain=action_domain(step)
    if domain is None:return None
    missing=[];blocked=[]
    value={'domain':domain,'unknown':missing,'blocked':blocked}
    if domain=='political' or 'political_power_cost' in step:
        cost=step.get('political_power_cost')
        fact=state.get('facts',{}).get('country.politics.political_power',{})
        amount=fact.get('value') if (state['usable'] and fact.get('status') in ('observed','calculated') and fact.get('source')) else None
        if not is_json_number(amount):amount=None
        affordable=None if cost is None or amount is None else amount>=cost
        value['political_power']={'cost':cost,'available':amount,'affordable':affordable,
                                  'cost_basis':'guide_declared' if cost is not None else 'unknown'}
        if cost is None:missing.append('political_cost_unknown')
        if amount is None:missing.append('political_power_unknown')
        if affordable is False:blocked.append('insufficient_political_power')
    if 'requirements' in step:
        eligible=evaluate_condition(step['requirements'],state,checks)
        if eligible is False:blocked.append('action_requirements_false')
        elif eligible is None:missing.append('action_requirements_unknown')
    else:
        missing.append('action_requirements_unknown')
    if domain=='appointment':missing.append('commander_assignment_unobserved')
    if domain=='project':
        # A statically resolved specialization is not proof of an available
        # compatible native facility. A declared condition can verify that fact.
        missing.append('facility_compatibility_unknown')
    value['value']=False if blocked else None if missing else True
    return value


def sequence_notice(runtime):
    key=runtime.get('action_sequence',{}).get('political')
    row=next((r for r in runtime.get('steps',[]) if r['key']==key),None)
    if not row:return None
    details=row.get('action_readiness',{})
    if 'insufficient_political_power' in details.get('blocked',[]):
        power=details['political_power']
        return {'code':'insufficient_political_power','message':f"Prima azione politica della guida: {key}. Potere politico {power['available']} / {power['cost']} indicati dalla guida; attendo senza saltare alla nomina successiva."}
    if row.get('reason') in ('dependency_unknown','dependency_not_completed'):
        return {'code':row['reason'],'message':'La prossima azione politica attende il prerequisito dichiarato dalla guida: '+', '.join(row.get('blocked_by',[]))}
    if row.get('status') in ('waiting','unverifiable'):
        return {'code':row['reason'],'message':'La prossima azione politica non è ancora verificabile o disponibile secondo i criteri della guida. La sequenza conserva '+key+'.'}
    return None
