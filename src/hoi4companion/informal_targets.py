"""Resolve complete literal commands through the immutable native catalog."""

def named_command(text,index,country):
    from .backend_core import _request
    from .resolution import catalog_index
    index=catalog_index(index)
    return _request(0,'import_named_command',text=text,country=country,**index.native_import_args())


def production_command(text,index,country,known_variants=None):
    from .backend_core import _request
    from .resolution import catalog_index
    index=catalog_index(index)
    return _request(0,'import_production_command',text=text,country=country,library=index.reference_designs,
                    declared=known_variants,**index.native_import_args())
