"""Whole-guide identity proofs owned by the native backend; no live facts."""
from .backend_core import _request
from .resolution import catalog_index

def infer_focus_scope(guide,catalog):
    index=catalog_index(catalog)
    return _request(0,'focus_infer_scope',guide=guide,**index.native_import_args())

def resolve_focus_routes(guide,catalog):
    index=catalog_index(catalog)
    result=_request(0,'focus_resolve_routes',guide=guide,**index.native_import_args())
    return result['resolutions'],result['route'],result['errors']
