"""Promote explicit source commands through the deterministic native compiler.
Unknown methods and conditions remain visible; no strategy is generated.
"""

def promote_operations(guide,catalog):
    from .backend_core import _request,_publish_in_place
    from .resolution import catalog_index
    index=catalog_index(catalog)
    result=_request(0,'import_promote_operations',guide=guide,library=index.reference_designs,**index.native_import_args())
    _publish_in_place(guide,result['guide'])
    return result['changes']
