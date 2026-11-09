"""Compile bounded source timing through the deterministic native backend."""

def compile_timing(text,catalog=None,country=None,*,scope=None,currency=None,depth=0):
    from .backend_core import _request
    from .resolution import catalog_index
    index=catalog_index(catalog) if catalog is not None else None
    context=index.native_import_args() if index is not None else {}
    return _request(0,'import_timing',text=text,country=country,scope=scope,currency=currency,depth=depth,**context)
