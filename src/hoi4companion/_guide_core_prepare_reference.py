"""Rust guide decisions with transitional adapters for existing qualified data.

The DLL retains immutable compiled guide graphs, never live facts or progress.
This is migration stage 2, not a claim that the Python backend has been removed.
"""
import atexit
from collections import OrderedDict
import ctypes
from importlib.machinery import EXTENSION_SUFFIXES
import json
import os
from pathlib import Path
import sys
from threading import RLock

MAX_BYTES=16*1024*1024
_lock=RLock()
_dll=None
_compiled=OrderedDict()


class GuideCoreUnavailable(ValueError):
    pass


def _library():
    global _dll
    if _dll is not None:return _dll
    override=os.environ.get('HOI4_GUIDE_CORE_LIBRARY')
    folder=Path(__file__).parent/'_native'
    suffix='.dll' if sys.platform=='win32' else '.dylib' if sys.platform=='darwin' else '.so'
    candidates=[Path(override)] if override else [folder/('hoi4companion_guide_core'+x) for x in dict.fromkeys([suffix,*EXTENSION_SUFFIXES])]
    path=next((p for p in candidates if p.is_file()),None)
    if path is None:raise GuideCoreUnavailable('Rust guide core is not installed. Run scripts/build-guide-core.ps1.')
    try:
        dll=ctypes.CDLL(str(path.resolve()))
        dll.hoi4_guide_schema_version.argtypes=[];dll.hoi4_guide_schema_version.restype=ctypes.c_uint32
        if dll.hoi4_guide_schema_version()!=1:raise ValueError('Unsupported guide core ABI')
        for name,args in [('hoi4_guide_info',[]),('hoi4_guide_compile',[ctypes.c_void_p,ctypes.c_size_t]),
                          ('hoi4_guide_evaluate',[ctypes.c_uint64,ctypes.c_void_p,ctypes.c_size_t])]:
            fn=getattr(dll,name);fn.argtypes=args;fn.restype=ctypes.c_void_p
        dll.hoi4_guide_release.argtypes=[ctypes.c_uint64];dll.hoi4_guide_release.restype=ctypes.c_bool
        dll.hoi4_guide_free.argtypes=[ctypes.c_void_p];dll.hoi4_guide_free.restype=None
    except (OSError,AttributeError,ValueError) as exc:raise GuideCoreUnavailable('Rust guide core could not be loaded: '+str(exc)) from exc
    _dll=dll
    return dll


def _reply(dll,ptr):
    if not ptr:raise GuideCoreUnavailable('Rust guide core returned no result')
    try:value=json.loads(ctypes.string_at(ptr))
    finally:dll.hoi4_guide_free(ptr)
    if not isinstance(value,dict) or value.get('schema_version')!=1:raise GuideCoreUnavailable('Invalid guide core response')
    if 'error' in value:raise ValueError('Rust guide core: '+str(value['error']))
    if 'result' not in value:raise GuideCoreUnavailable('Incomplete guide core response')
    return value['result']


def _encode(value):
    data=json.dumps(value,ensure_ascii=False,separators=(',',':'),allow_nan=False).encode('utf-8')
    if len(data)>MAX_BYTES:raise ValueError('Guide core input exceeds limit')
    return data


def engine_info():
    with _lock:
        dll=_library()
        return _reply(dll,dll.hoi4_guide_info())


def clear_compiled():
    with _lock:
        if _dll is not None:
            for handle in _compiled.values():_dll.hoi4_guide_release(handle)
        _compiled.clear()


atexit.register(clear_compiled)


def prepare(guide,state,progress=None):
    """Existing adapters remain authoritative for normalization during migration.

    No completion, admission, priority or proposed action is supplied to Rust.
    """
    from .guide import normalise_guide
    from ._contracts_reference import normalise_state
    from ._guide_runtime_reference import _progress
    from ._campaign_conditions_reference import context as campaign_context
    from ._construction_progress_reference import validate_baselines,construction_context
    from ._production_progress_reference import production_context,selector_for,selector_identity
    from .foreign_interactions import predicate,token
    guide=normalise_guide(guide);state=normalise_state(state)
    country=state['facts'].get('country',{}).get('value') if state['usable'] else None
    tree=state['facts'].get('focus.tree',{}).get('value') if state['usable'] else None
    scope=guide['applies_to'];mode='live' if state['usable'] else 'preview'
    if state['usable']:
        if scope.get('country') and country is not None and country!=scope['country']:mode='wrong_country'
        elif scope.get('tree_id') and tree is not None and tree!=scope['tree_id']:mode='wrong_tree'
        elif scope.get('country') and country is None or scope.get('tree_id') and tree is None:mode='preview'
    status,run_id,checks,started=_progress(guide,state,progress)
    baselines=validate_baselines(progress.get('baselines',{})) if progress is not None else {}
    if mode in ('wrong_country','wrong_tree'):checks={};started={}
    context=state if mode=='live' else {'schema_version':1,'usable':False,'session_id':None,'facts':{}}
    context=campaign_context(context,progress.get('campaign_conditions',{}) if status=='accepted' else None)
    context,construction=construction_context(guide,context,baselines if status=='accepted' else {})
    context,production=production_context(guide,context,baselines if status=='accepted' else {})
    selectors={};foreign={};explicit=[]
    for rows in guide['tracks'].values():
        for step in rows:
            if 'depends_on' in step or 'depends_on_started' in step:explicit.append(step['key'])
            params=step['action'].get('params',{});node=predicate(params)
            if node:foreign[step['key']]=token(step,node)
            if params.get('production_revision'):
                selected=[]
                for target in params.get('production_targets',[params]):
                    selector=selector_for({'action':{'params':target}})
                    selected.append({'identity':selector_identity(selector),'category':selector['category']} if selector else None)
                selectors[step['key']]=selected
    payload={'schema_version':1,'state':context,'original_state':state,'progress_status':status,'run_id':run_id,
             'checks':checks,'started':started,'explicit_dependencies':explicit,'construction':construction,
             'production':production,
             'research_bindings':progress.get('research_bindings',{}) if status=='accepted' and isinstance(progress.get('research_bindings',{}),dict) else {},
             'research_options':progress.get('research_options',{}) if status=='accepted' else {},
             'foreign_tokens':foreign,'selectors':selectors}
    definition={k:guide[k] for k in ('id','applies_to','dependency_policy','phases','tracks') if k in guide}
    definition.setdefault('id','')
    # Display/source copy does not participate in compiled decision identity.
    # Full validation above still checks those fields; the caller's data is intact.
    definition['tracks']={track:[{**{k:v for k,v in step.items() if k not in ('evidence','title','name','note')},
        'action':{**step['action'],'params':{k:v for k,v in step['action'].get('params',{}).items()
            if k not in ('instructions','source_action','reference_images','design_reference')}}} for step in rows]
        for track,rows in guide['tracks'].items()}
    return definition,payload


def evaluate(guide,state,progress=None):
    definition,payload=prepare(guide,state,progress)
    compiled_key=_encode(definition);data=_encode(payload)
    with _lock:
        dll=_library();handle=_compiled.get(compiled_key)
        if handle is None:
            if len(_compiled)>=16:
                _,old=_compiled.popitem(last=False);dll.hoi4_guide_release(old)
            buffer=ctypes.create_string_buffer(compiled_key)
            handle=_reply(dll,dll.hoi4_guide_compile(buffer,len(compiled_key)))['handle']
            _compiled[compiled_key]=handle
        else:_compiled.move_to_end(compiled_key)
        buffer=ctypes.create_string_buffer(data)
        result=_reply(dll,dll.hoi4_guide_evaluate(handle,buffer,len(data)))
    if os.environ.get('HOI4_GUIDE_CORE_VERIFY')=='1':
        from .guide_runtime import evaluate_python
        expected=evaluate_python(guide,state,progress)
        if result!=expected:raise ValueError('Guide core comparison failed; no native result admitted')
    return result
