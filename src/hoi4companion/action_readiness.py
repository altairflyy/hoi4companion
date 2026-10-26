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
    resources=[];affordability=[]
    value={'domain':domain,'unknown':missing,'blocked':blocked,'resources':resources,
           'requirements_declared':'requirements' in step}
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
        affordability.append(affordable)
    costs=step.get('resource_costs')
    if domain=='project' and costs is None:
        missing.append('project_cost_unknown');affordability.append(None)
    for key,cost in (costs or {}).items():
        fact=state.get('facts',{}).get(key,{})
        amount=fact.get('value') if (state['usable'] and fact.get('status') in ('observed','calculated') and fact.get('source')) else None
        if not is_json_number(amount):amount=None
        affordable=None if amount is None else amount>=cost
        resources.append({'key':key,'cost':cost,'available':amount,'affordable':affordable})
        affordability.append(affordable)
        if affordable is False and 'insufficient_resource' not in blocked:blocked.append('insufficient_resource')
        elif affordable is None and 'resource_balance_unknown' not in missing:missing.append('resource_balance_unknown')
    value['resources_available']=False if False in affordability else None if None in affordability else True
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


def hud_resource_ready(value):
    """Unknown funds are not permission to draw an actionable yellow border.

    Facility/role identity is checked by native navigation. Undeclared dynamic
    requirements remain a diagnostic, while an explicit unknown requirement
    withholds the recommendation as well as an unknown or insufficient balance.
    """
    return (isinstance(value,dict) and value.get('resources_available') is True
            and not value.get('blocked') and not (value.get('requirements_declared')
                and 'action_requirements_unknown' in value.get('unknown',[])))


def sequence_notice(runtime):
    key=runtime.get('action_sequence',{}).get('political')
    row=next((r for r in runtime.get('steps',[]) if r['key']==key),None)
    if not row:
        keys=runtime.get('action_sequence',{}).get('projects',[])
        row=next((r for r in runtime.get('steps',[]) if r['key'] in keys and r['status']!='active'),None)
    if not row:return None
    details=row.get('action_readiness',{})
    resources=details.get('resources',[])
    insufficient=[r for r in resources if r['affordable'] is False]
    if insufficient:
        required='; '.join(f"{r['key']}: {r['available']} / {r['cost']}" for r in insufficient)
        return {'code':'insufficient_resource','message':'HUD in attesa delle risorse per '+row['key']+'. '+required}
    if 'project_cost_unknown' in details.get('unknown',[]) or 'resource_balance_unknown' in details.get('unknown',[]):
        return {'code':'resource_unverifiable','message':'Costo o saldo dei punti non qualificato per '+row['key']+': nessuna cornice HUD finché il dato resta sconosciuto.'}
    if 'insufficient_political_power' in details.get('blocked',[]):
        power=details['political_power']
        return {'code':'insufficient_political_power','message':f"Prima azione politica della guida: {key}. Potere politico {power['available']} / {power['cost']} indicati dalla guida; attendo senza saltare alla nomina successiva."}
    if any(reason in details.get('unknown',[]) for reason in ('political_cost_unknown','political_power_unknown')):
        return {'code':'political_resource_unverifiable','message':'Potere politico o costo non qualificato per '+row['key']+': HUD in attesa, senza proporre una nomina successiva.'}
    if row.get('reason') in ('dependency_unknown','dependency_not_completed'):
        return {'code':row['reason'],'message':'La prossima azione politica attende il prerequisito dichiarato dalla guida: '+', '.join(row.get('blocked_by',[]))}
    if row.get('status') in ('waiting','unverifiable'):
        return {'code':row['reason'],'message':'La prossima azione politica non è ancora verificabile o disponibile secondo i criteri della guida. La sequenza conserva '+key+'.'}
    return None
