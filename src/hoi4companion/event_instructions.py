"""Resolve explicit event answers without choosing a strategy or inferring outcomes."""

def event_command(text,index,country):
    from .backend_core import _request
    from .resolution import catalog_index
    index=catalog_index(index)
    return _request(0,'import_event_command',text=text,country=country,**index.native_import_args())
