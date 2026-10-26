"""Small, versioned authoring interface compiled into the existing Guide v1.

No acquisition, name guessing, execution or persistence. Catalog resolution and
live predicates remain the responsibility of the existing importer/binders.
"""
from copy import deepcopy
import math

from .guide import normalise_guide

FORMAT = 'hoi4-guide-authoring'
VERSION = 1
CATEGORIES = {'construction','production','politics','characters','army','templates','projects','checkpoints'}
# This table also supplies the prompt and the public capability reference.
SPECS = {
    'focus': ('select_focus', None, 'target', 'target'),
    'research': ('research', None, 'target upgrade_from', 'target'),
    'special_project': ('special_project', 'projects', 'target', 'target'),
    'build': ('manual', 'construction', 'building state quantity baseline province queue_priority', 'building state quantity'),
    'production': ('manual', 'production', 'equipment factories variant production_category lines', ''),
    'law': ('manual', 'politics', 'target group', 'target'),
    'idea': ('manual', 'politics', 'target', 'target'),
    'appoint': ('manual', 'characters', 'person advisor trait role', ''),
    'event': ('manual', 'politics', 'event option', 'event option'),
    'decision': ('manual', 'politics', 'target country', 'target'),
    'create_agency': ('manual', 'projects', '', ''),
    'agency_upgrade': ('manual', 'projects', 'target', 'target'),
    'intelligence_operation': ('manual', 'projects', 'operation country', 'operation'),
    'mio': ('manual', 'production', 'organization equipment', 'organization'),
    'template': ('manual', 'templates', 'template', 'template'),
    'design': ('manual', 'templates', 'name', 'name'),
    'army': ('manual', 'army', 'task', 'task'),
    'diplomacy': ('manual', 'politics', 'task country', 'task'),
    'instruction': ('manual', None, 'category', 'category instructions'),
    'reminder': ('manual', None, 'category', 'instructions'),
    'free': ('free', None, 'lane', ''),
    'advanced_v1': (None, None, 'step', 'step'),
}
POLICIES = {'title','note','confirmation','phase','depends_on','depends_on_started','political_power_cost','requirements','when','skip_when','priority','priority_when','completion','started_when'}
COMMON = POLICIES | {'key','type','evidence','timing','instructions','source_seconds','game_date','details'}
GENERATED = {'verification','production_selector','political_binding','kind','entity','target','completion','started_when'}
IDENTITIES = {'building_id','building_name','state_id','state_name','quantity','quantity_mode','target_level','target_quantity',
              'equipment_id','equipment_name','assigned_factories','requested_factories','production_targets','law_id','law_name',
              'idea_id','idea_name','person_id','person_name','advisor_id','event_id','event_name','event_option_id','event_option',
              'decision_id','decision_name','project_id','project_name','operation_id','mio_name'}
IDENTITIES |= {'baseline_level','province_id','queue_priority','variant_name','law_group','trait_name','role',
               'template_name','design_name','task','operation_name','upgrade_name','target_country','upgrade_from'}


def action_reference():
    return [{'type':name,'fields':fields.split(),'required':required.split(),
             'category':category,'runtime_kind':kind} for name,(kind,category,fields,required) in SPECS.items()]


def _error(key, message):
    raise ValueError(f'{key}: {message}')


def _integer(value, minimum=0):
    return type(value) is int and minimum <= value <= 9007199254740991


def _text(value, key, field):
    if not isinstance(value,str) or not value.strip() or len(value)>4096:
        _error(key, f'{field} mancante o non valido')
    return value


def _ref(value, key, field):
    if isinstance(value,str):
        return 'name',_text(value,key,field),None
    if not isinstance(value,dict) or set(value)-{'id','name','scope'} or (('id' in value)==('name' in value)):
        _error(key, f'{field}: usa un nome oppure un riferimento con id o name')
    kind='id' if 'id' in value else 'name'
    target=value[kind]
    if field=='state' and kind=='id' and _integer(target,1):target=str(target)
    target=_text(target,key,field)
    scope=value.get('scope')
    if 'scope' in value:scope=_text(scope,key,field+'.scope')
    return kind,target,scope


def _put_ref(params, value, key, field, id_field, name_field):
    kind,target,scope=_ref(value,key,field)
    if scope is not None:_error(key,f'{field}: scope ammesso solo per focus')
    params[id_field if kind=='id' else name_field]=target


def _details(step, key, *, generic=False):
    value=step.get('details',{})
    if not isinstance(value,dict):_error(key,'details deve essere un oggetto')
    forbidden=GENERATED | {'category','timing','instructions','source_seconds','game_date'}
    if not generic:forbidden |= IDENTITIES
    if set(value)&forbidden or any(k.startswith('guide.') for k in value if isinstance(k,str)):
        _error(key,'details contiene campi riservati o in conflitto: '+', '.join(sorted(set(value)&forbidden)))
    def visit(item):
        if isinstance(item,dict):
            if set(item)&{'verification','production_selector','political_binding'}:_error(key,'details contiene campi interni generati')
            for child in item.values():visit(child)
        elif isinstance(item,list):
            for child in item:visit(child)
    visit(value)
    return deepcopy(value)


def _factories(value, key):
    if (not isinstance(value,dict) or set(value)-{'value','basis','mode'}
            or not _integer(value.get('value')) or value.get('basis','requested') not in ('requested','assigned')
            or value.get('mode','total') not in ('total','add')):
        _error(key,'factories: richiesti value intero, basis requested/assigned e mode total/add')
    if value.get('mode')=='add' and value['value']==0:_error(key,'factories: un incremento deve essere positivo')
    return ('requested_factories' if value.get('basis','requested')=='requested' else 'assigned_factories',
            value['value'],'additional' if value.get('mode','total')=='add' else 'target_total')


def _production_line(value, key):
    allowed={'equipment','factories','variant','production_category'}
    if not isinstance(value,dict) or set(value)-allowed or 'equipment' not in value or 'factories' not in value:
        _error(key,'produzione: servono equipment e factories; campi ammessi '+', '.join(sorted(allowed)))
    params={}
    _put_ref(params,value['equipment'],key,'equipment','equipment_id','equipment_name')
    field,count,mode=_factories(value['factories'],key)
    params[field]=count
    if mode=='additional':params['quantity_mode']=mode
    if 'variant' in value:params['variant_name']=_text(value['variant'],key,'variant')
    if 'production_category' in value:
        if value['production_category'] not in ('military','naval'):_error(key,'production_category: military oppure naval')
        params['production_category']=value['production_category']
    return params


def _compile_step(step,index):
    if not isinstance(step,dict):_error(str(index),'step non valido')
    key=step.get('key',f'step-{index+1:04d}')
    _text(key,str(index),'key')
    typ=step.get('type')
    if not isinstance(typ,str) or typ not in SPECS:_error(key,'type sconosciuto: '+str(typ))
    kind,category,fields,required=SPECS[typ]
    unknown=set(step)-COMMON-set(fields.split())
    if unknown:_error(key,'campi non riconosciuti: '+', '.join(sorted(unknown)))
    for field in required.split():
        if field not in step or step[field] is None:_error(key,'manca '+field)
    if typ=='advanced_v1':
        if set(step)-{'type','step','key','title','evidence'}:_error(key,'advanced_v1: conserva le regole nello step v1')
        original=deepcopy(step['step'])
        if not isinstance(original,dict) or not isinstance(original.get('action'),dict):_error(key,'step v1 non valido')
        for field in ('key','title','evidence'):
            if field in step:
                if field in original and step[field]!=original[field]:_error(key,'advanced_v1: '+field+' in conflitto')
                original[field]=deepcopy(step[field])
        original.setdefault('key',key)
        lane='focus' if original['action'].get('kind')=='select_focus' else 'research' if original['action'].get('kind') in ('research','free') else 'operations'
        return lane,original
    params=_details(step,key,generic=typ in ('instruction','reminder'))
    if category:params['category']=category
    for field in ('timing','source_seconds','game_date'):
        if field in step:
            value=step[field]
            if field=='source_seconds':
                if type(value) not in (int,float) or not math.isfinite(value) or value<0:_error(key,'source_seconds deve essere un tempo positivo o zero')
            else:_text(value,key,field)
            params[field]=deepcopy(value)
    if 'instructions' in step:
        if (not isinstance(step['instructions'],list) or not step['instructions']
                or any(not isinstance(s,str) or not s.strip() for s in step['instructions'])):_error(key,'instructions deve contenere frasi non vuote')
        params['instructions']=deepcopy(step['instructions'])
    row={k:deepcopy(v) for k,v in step.items() if k in POLICIES|{'evidence'}}
    row['key']=key
    action={'kind':kind}
    if typ in ('focus','research','special_project'):
        _,target,scope=_ref(step['target'],key,'target')
        if scope is not None and typ!='focus':_error(key,'scope ammesso solo per focus')
        action['entity']={'kind':{'focus':'focus','research':'technology','special_project':'project'}[typ],'id':target}
        if scope:action['entity']['scope']=scope
        if 'upgrade_from' in step:params['upgrade_from']=_text(step['upgrade_from'],key,'upgrade_from')
    elif typ=='build':
        _put_ref(params,step['building'],key,'building','building_id','building_name')
        _put_ref(params,step['state'],key,'state','state_id','state_name')
        if 'state_id' in params:
            value=params['state_id']
            if not value.isascii() or not value.isdigit() or not _integer(int(value),1):_error(key,'state.id deve essere intero positivo')
            params['state_id']=int(value)
        q=step['quantity']
        if not isinstance(q,dict) or set(q)!={'mode','value'} or q.get('mode') not in ('add','total') or not _integer(q.get('value'),1):
            _error(key,'quantity: servono mode add/total e value intero positivo')
        params.update(quantity=q['value'],quantity_mode='additional' if q['mode']=='add' else 'target_total')
        for src,dst,minimum in [('baseline','baseline_level',0),('province','province_id',1),('queue_priority','queue_priority',1)]:
            if src in step:
                if not _integer(step[src],minimum):_error(key,src+' deve essere intero valido')
                params[dst]=step[src]
    elif typ=='production':
        if 'lines' in step:
            if any(field in step for field in ('equipment','factories','variant','production_category')):_error(key,'produzione: scegli lines oppure una singola linea')
            lines=step['lines']
            if not isinstance(lines,list) or not 0<len(lines)<=64:_error(key,'lines: richieste da 1 a 64 linee')
            params['production_targets']=[_production_line(v,key+f'.lines[{i}]') for i,v in enumerate(lines)]
            if any(v.get('quantity_mode')=='additional' for v in params['production_targets']):_error(key,'lines richiede totali, non incrementi')
        else:params.update(_production_line({k:step[k] for k in ('equipment','factories','variant','production_category') if k in step},key))
    elif typ in ('law','idea','decision'):
        _put_ref(params,step['target'],key,'target',typ+'_id',typ+'_name')
        if 'group' in step:params['law_group']=_text(step['group'],key,'group')
    elif typ=='appoint':
        if not any(step.get(k) for k in ('person','advisor','trait')):_error(key,'manca person, advisor o trait')
        if 'person' in step and 'advisor' in step:_error(key,'scegli person oppure advisor')
        if 'person' in step:_put_ref(params,step['person'],key,'person','person_id','person_name')
        if 'advisor' in step:_put_ref(params,step['advisor'],key,'advisor','advisor_id','person_name')
        if 'trait' in step:params['trait_name']=_text(step['trait'],key,'trait')
        if 'role' in step:params['role']=_text(step['role'],key,'role')
        if 'trait' in step and not any(k in step for k in ('person','advisor','role')):_error(key,'manca role per la scelta tramite trait')
    elif typ=='event':
        _put_ref(params,step['event'],key,'event','event_id','event_name')
        _put_ref(params,step['option'],key,'option','event_option_id','event_option')
    elif typ=='create_agency':params['operation_id']='create_intelligence_agency'
    elif typ=='agency_upgrade':params.update(operation_id='upgrade_intelligence_agency',upgrade_name=_text(step['target'],key,'target'))
    elif typ=='intelligence_operation':params['operation_name']=_text(step['operation'],key,'operation')
    elif typ=='mio':
        params['mio_name']=_text(step['organization'],key,'organization')
        if 'equipment' in step:_put_ref(params,step['equipment'],key,'equipment','equipment_id','equipment_name')
    elif typ=='template':params['template_name']=_text(step['template'],key,'template')
    elif typ=='design':params['design_name']=_text(step['name'],key,'name')
    elif typ in ('army','diplomacy'):params['task']=_text(step['task'],key,'task')
    elif typ in ('instruction','reminder'):
        category=step.get('category','checkpoints')
        if category not in CATEGORIES:_error(key,'category non valida')
        params['category']=category
        if typ=='reminder':row['tracking']='reminder'
    if 'country' in step:params['target_country']=_text(step['country'],key,'country')
    if params:action['params']=params
    row['action']=action
    lane='focus' if typ=='focus' else 'research' if typ in ('research','free') else 'operations'
    if typ=='free':
        lane=step.get('lane','research')
        if lane not in ('focus','research'):_error(key,'lane: focus oppure research')
    return lane,row


def compile_document(document):
    """Return an ordinary import v1 document, preserving sources and policies."""
    if (not isinstance(document,dict) or document.get('format')!=FORMAT
            or type(document.get('authoring_schema_version')) is not int or document['authoring_schema_version']!=VERSION):
        raise ValueError('Formato authoring o versione non supportata')
    if set(document)-{'format','authoring_schema_version','sources','guide'}:raise ValueError('Campi authoring non riconosciuti')
    value=document.get('guide')
    if not isinstance(value,dict) or 'tracks' in value:raise ValueError('Authoring: usa guide.steps, senza tracks')
    allowed={'id','title','subtitle','applies_to','sources','phases','assumptions','steps','start_date','research_baseline','source_inventory'}
    if set(value)-allowed:raise ValueError('Metadati authoring non riconosciuti: '+', '.join(sorted(set(value)-allowed)))
    for field in ('id','title'):_text(value.get(field),'guide',field)
    steps=value.get('steps')
    if not isinstance(steps,list) or not 0<len(steps)<=4096:raise ValueError('Authoring: richiesti da 1 a 4096 step')
    sources=document.get('sources')
    if not isinstance(sources,list) or not 1<=len(sources)<=8:raise ValueError('Authoring: richieste da 1 a 8 fonti')
    guide=deepcopy(value);guide.pop('steps');guide.setdefault('schema_version',1)
    if 'sources' not in guide:
        guide['sources']=[{k:deepcopy(s[k]) for k in ('id','type','title','url') if k in s} for s in sources if isinstance(s,dict)]
    guide['tracks']={'focus':[],'research':[],'operations':[]}
    for i,step in enumerate(steps):
        lane,row=_compile_step(step,i);guide['tracks'][lane].append(row)
    guide=normalise_guide(guide)
    return {'format':'hoi4-guide-import','import_schema_version':1,'sources':deepcopy(sources),'guide':guide}
