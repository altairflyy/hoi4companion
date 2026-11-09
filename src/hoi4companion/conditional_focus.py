"""Native compiler of declared bypass/fallback notation; never a live observation."""
from .backend_core import _request,_publish_in_place

def compile_conditional_focuses(guide,index):
    result=_request(0,'focus_compile_conditionals',guide=guide,**index.native_import_args())
    # Import orchestration already retains row references. Preserve their
    # identity while publishing the compiler's changes.
    for track,rows in guide['tracks'].items():
        for row,compiled in zip(rows,result['guide']['tracks'][track]):
            _publish_in_place(row,compiled)
    return result['resolutions']
