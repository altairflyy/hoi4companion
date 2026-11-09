"""Read-only game-script criteria and optional, authored focus alternatives."""
from copy import deepcopy
import math
import re
from .clausewitz import Node

NUMBERS={'has_war_support':'country.metrics.war_support','has_stability':'country.metrics.stability',
         'political_power':'country.politics.political_power','has_army_experience':'country.experience.army',
         'has_navy_experience':'country.experience.navy','has_air_experience':'country.experience.air'}
MEMBERS={'has_completed_focus':'focus.completed','has_tech':'research.completed','has_idea':'politics.ideas'}
BOOLS={'has_war':'country.diplomacy.at_war','is_subject':'country.diplomacy.is_subject'}
TEXT={'tag':'country','original_tag':'country.original_tag','has_government':'country.politics.government'}


def compile_condition(node, scripts=None, dlcs=None, stack=(), _depth=0, _budget=None):
    scripts=scripts or {};parts=[]
    if not isinstance(node,Node):return {'op':'unknown','trigger':'invalid_condition_block'}
    _budget=[0] if _budget is None else _budget
    if _depth>20 or _budget[0]>=768:return {'op':'unknown','trigger':'criteria_processing_limit'}
    for key,value in node.entries:
        _budget[0]+=1
        if _budget[0]>=768:
            parts.append({'op':'unknown','trigger':'criteria_processing_limit'});break
        match=re.fullmatch(r'(.+?)(?:\s+(>=|<=|>|<|!=|=))?',key)
        if not match:
            parts.append({'op':'unknown','trigger':'invalid_condition_entry'});continue
        name,operator=match.groups();operator=operator or '=';upper=name.upper()
        if name=='always' and value in ('yes','no'):part={'op':'const','value':value=='yes'}
        elif upper in ('AND','OR','NOT','NAND','NOR') and isinstance(value,Node):
            children=[]
            for k,v in value.entries:
                children.append(compile_condition(Node([(k,v)]),scripts,dlcs,stack,_depth+1,_budget))
                if _budget[0]>=768:break
            if upper=='NOT' and len(children)!=1:part={'op':'unknown','trigger':'NOT with multiple predicates'}
            else:
                group={'op':'any' if upper in ('OR','NOR') else 'all','args':children}
                part={'op':'not','arg':children[0] if upper=='NOT' else group} if upper in ('NOT','NAND','NOR') else group
        elif name=='custom_trigger_tooltip' and isinstance(value,Node):
            part=compile_condition(Node([(k,v) for k,v in value.entries if k!='tooltip']),scripts,dlcs,stack,_depth+1,_budget)
        elif upper=='IF' and isinstance(value,Node) and isinstance(value.first('limit'),Node):
            part={'op':'if','condition':compile_condition(value.first('limit'),scripts,dlcs,stack,_depth+1,_budget),
                  'then':compile_condition(Node([(k,v) for k,v in value.entries if k!='limit']),scripts,dlcs,stack,_depth+1,_budget)}
        elif name in scripts and value in ('yes','no') and name not in stack and len(stack)<24:
            part=compile_condition(scripts[name],scripts,dlcs,(*stack,name),_depth+1,_budget)
            if value=='no':part={'op':'not','arg':part}
        elif name=='has_dlc' and isinstance(value,str) and dlcs is not None and dlcs.enabled(value) is not None:
            part={'op':'const','value':dlcs.enabled(value)}
        elif name in NUMBERS and isinstance(value,str) and operator in ('>','<','>=','<='):
            try:number=float(value)
            except ValueError:number=float('nan')
            part=({'op':'compare','fact':NUMBERS[name],'test':operator,'value':number,'trigger':name}
                  if math.isfinite(number) else {'op':'unknown','trigger':key})
        elif name in MEMBERS and isinstance(value,str) and operator=='=':
            part={'op':'compare','fact':MEMBERS[name],'test':'contains','value':value,'trigger':name}
        elif name in BOOLS and value in ('yes','no') and operator=='=':
            part={'op':'compare','fact':BOOLS[name],'test':'=','value':value=='yes','trigger':name}
        elif name in TEXT and isinstance(value,str) and operator in ('=','!='):
            part={'op':'compare','fact':TEXT[name],'test':operator,'value':value,'trigger':name}
        else:
            part={'op':'unknown','trigger':key}
            if isinstance(value,str):part['value']=value
        parts.append(part)
    return {'op':'all','args':parts}


def compile_rules(node):
    return {'schema_version':1,'available':compile_condition(node.first('available',Node())),
            'branch':compile_condition(node.first('allow_branch',Node()))}


def _expand_batch(rules,scripts,dlcs):
    from .backend_core import _request
    from .script_document import encode
    return _request(0,'source_expand_rules',rules=rules,scripts={key:encode(node) for key,node in scripts.items()},
                    dlcs={'items':dlcs.items,'complete':dlcs.complete} if dlcs is not None else None)


def expand_rules(rules,scripts,dlcs):
    return _expand_batch({'one':rules},scripts,dlcs)['one']


def expand_tree_rules(trees,scripts,dlcs):
    indexed={str(i):focus for i,focus in enumerate(f for tree in trees.values() for f in tree.focuses.values() if f.criteria)}
    if not indexed:return
    result=_expand_batch({key:focus.criteria for key,focus in indexed.items()},scripts,dlcs)
    for key,focus in indexed.items():focus.criteria=result[key]


def validate_rules(rules):
    if not isinstance(rules,dict) or set(rules)!={'schema_version','available','branch','prerequisites','mutex','conflicts'} or type(rules.get('schema_version')) is not int or rules['schema_version']!=1:
        raise ValueError('Invalid focus criteria metadata')
    budget=[0]
    def visit(n,depth=0):
        budget[0]+=1
        if depth>32 or budget[0]>4096 or not isinstance(n,dict):raise ValueError('Invalid focus criterion')
        op=n.get('op')
        if op in ('all','any'):
            if set(n)!={'op','args'} or not isinstance(n['args'],list):raise ValueError('Invalid focus criteria group')
            for x in n['args']:visit(x,depth+1)
        elif op=='not':
            if set(n)!={'op','arg'}:raise ValueError('Invalid focus criteria negation')
            visit(n['arg'],depth+1)
        elif op=='if':
            if set(n)!={'op','condition','then'}:raise ValueError('Invalid focus criteria implication')
            visit(n['condition'],depth+1);visit(n['then'],depth+1)
        elif op=='const':
            if set(n)!={'op','value'} or type(n['value']) is not bool:raise ValueError('Invalid focus constant')
        elif op=='unknown':
            if set(n)-{'op','trigger','value'} or not isinstance(n.get('trigger'),str) or 'value' in n and not isinstance(n['value'],str):raise ValueError('Invalid unknown focus predicate')
        elif op=='compare':
            if (set(n)!={'op','fact','test','value','trigger'} or not isinstance(n['fact'],str)
                    or n['test'] not in ('>','<','>=','<=','=','!=','contains')
                    or type(n['value']) not in (str,bool,int,float) or isinstance(n['value'],float) and not math.isfinite(n['value'])
                    or n['test'] in ('>','<','>=','<=') and type(n['value']) not in (int,float)
                    or n['test']=='contains' and not isinstance(n['value'],str)
                    or not isinstance(n['trigger'],str)):raise ValueError('Invalid focus comparison')
        else:raise ValueError('Unsupported focus criteria operator')
    visit(rules['available']);visit(rules['branch'])
    for field in ('mutex','conflicts'):
        if not isinstance(rules[field],list) or len(rules[field])>4096 or any(not isinstance(x,str) or not x for x in rules[field]):raise ValueError('Invalid focus exclusions')
    if (not isinstance(rules['prerequisites'],list) or len(rules['prerequisites'])>128
            or any(not isinstance(g,list) or not g or len(g)>512 or any(not isinstance(x,str) or not x for x in g) for g in rules['prerequisites'])):raise ValueError('Invalid focus prerequisite groups')


def assess(rules,state):
    checks=[]
    def fact(key):
        f=state.get('facts',{}).get(key,{})
        return f.get('value') if state.get('usable') and f.get('status') in ('observed','calculated') and f.get('source') else None
    def group(values,op):return (False if False in values else None if None in values else True) if op=='all' else (True if True in values else None if None in values else False)
    def evaluate(n):
        op=n['op']
        if op=='const':return n['value']
        if op in ('all','any'):return group([evaluate(x) for x in n['args']],op)
        if op=='not':
            v=evaluate(n['arg']);return None if v is None else not v
        if op=='if':
            a=evaluate(n['condition']);b=evaluate(n['then'])
            return True if a is False or b is True else b if a is True else None
        if op=='unknown':checks.append({'trigger':n['trigger'],'value':None,'reason':'unsupported_trigger'});return None
        actual=fact(n['fact']);expected=n['value'];test=n['test'];value=None
        if test=='contains':value=expected in actual if isinstance(actual,list) and all(isinstance(x,str) for x in actual) else None
        elif actual is not None:
            if test in ('>','<','>=','<='):
                if type(actual) in (int,float) and math.isfinite(actual):value={'>':actual>expected,'<':actual<expected,'>=':actual>=expected,'<=':actual<=expected}[test]
            elif type(actual) is type(expected) or type(actual) in (int,float) and type(expected) in (int,float):value=(actual==expected) if test=='=' else actual!=expected
        checks.append({'trigger':n['trigger'],'fact':n['fact'],'test':test,'required':expected,'actual':actual,'value':value})
        return value
    available=evaluate(rules['available']);branch=evaluate(rules['branch']);completed=fact('focus.completed')
    completed=completed if isinstance(completed,list) and all(isinstance(x,str) for x in completed) else None
    prereq=None if completed is None else all(any(p in completed for p in g) for g in rules['prerequisites'])
    excluded=None if completed is None else any(p in completed for p in rules['mutex'])
    value=group([available,branch,prereq,None if excluded is None else not excluded],'all')
    return {'value':value,'available':available,'branch':branch,'prerequisites':prereq,'excluded':excluded,'checks':checks,
            'reason':'focus_excluded' if excluded else 'focus_prerequisites_false' if prereq is False else 'focus_requirements_false' if value is False else 'focus_requirements_unknown' if value is None else 'focus_requirements_satisfied'}


def alternatives(rows,results,tracks,capacity):
    """Only later authored actions, verified available and route-compatible."""
    definitions={r['key']:r for _,r in rows}
    keys=[k for track in tracks.values() for k in track['order'] if definitions[k]['action']['kind']=='select_focus']
    pending=[k for k in keys if results[k]['status'] not in ('completed','not_applicable','active')
             and not definitions[k]['action'].get('params',{}).get('focus_optional')]
    if not pending:return None
    primary=pending[0];blocked=results[primary]
    result={'primary':primary,'blocked':blocked.get('focus_readiness',{}),'steps':[]}
    if capacity!=1 or blocked.get('focus_readiness',{}).get('value') is not False:return result
    protected={definitions[k]['action']['entity']['id'] for k in pending}
    for key in pending[1:]:
        row=definitions[key];item=results[key];rules=row.get('focus_criteria')
        if row['action']['entity']==definitions[primary]['action']['entity']:continue
        if (not rules or item['status']!='ready' or item.get('focus_readiness',{}).get('value') is not True
                or set(rules['conflicts'])&protected):continue
        result['steps'].append(key)
        # One free focus slot: the earliest verified independent guide action.
        break
    return result
