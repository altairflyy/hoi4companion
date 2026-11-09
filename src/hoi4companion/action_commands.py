"""Resolve complete compound commands without discarding their dependencies."""

def sequence(text,catalog,country):
    from .backend_core import _request
    from .resolution import catalog_index
    index=catalog_index(catalog)
    return _request(0,'import_action_sequence',text=text,country=country,**index.native_import_args())
