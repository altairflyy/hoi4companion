"""Single-call Rust preparation, graph cache and universal decisions.
The application host and incoming guide validation still migrate separately.
"""
import atexit
import os
from .backend_core import _request,BackendUnavailable


class GuideCoreUnavailable(ValueError):pass


def engine_info():
    return _request(0,'guide_info')


def clear_compiled():
    from . import backend_core
    if backend_core._dll is not None:_request(0,'guide_clear')


atexit.register(clear_compiled)


def prepare(guide,state,progress=None):
    from .guide import normalise_guide
    return tuple(_request(0,'guide_prepare',guide=normalise_guide(guide),state=state,progress=progress))


def evaluate(guide,state,progress=None):
    from .guide import normalise_guide
    try:result=_request(0,'guide_evaluate',guide=normalise_guide(guide),state=state,progress=progress)
    except BackendUnavailable as exc:raise GuideCoreUnavailable(str(exc)) from exc
    if os.environ.get('HOI4_GUIDE_CORE_VERIFY')=='1':
        from ._guide_runtime_reference import evaluate_python
        if result!=evaluate_python(guide,state,progress):raise ValueError('Guide core comparison failed; no native result admitted')
    return result
