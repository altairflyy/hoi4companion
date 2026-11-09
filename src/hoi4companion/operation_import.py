"""Connect structured imported orders to existing facts; never acquire live data."""
from .resolution import catalog_index


def operation_catalog(game, mods, labels, reference):
    from .backend_core import _request
    value=_request(0,'source_operations',game=str(game),mods=list(map(str,mods)),labels=labels,reference=reference)
    return value['rows'],value['warnings']


def _fact(key,test,value):
    return {'op':'fact','key':key,'test':test,'value':value}


def _automatic(condition):
    if not isinstance(condition,dict): return False
    if condition.get('op')=='fact': return True
    if condition.get('op') in ('all','any'): return all(_automatic(item) for item in condition['args'])
    if condition.get('op')=='not': return _automatic(condition['arg'])
    return False


def _integer(value,minimum=0):
    return type(value) is int and value>=minimum


def bind_operations(guide,catalog):
    from .backend_core import _request,_publish_in_place
    index=catalog_index(catalog)
    result=_request(0,'import_bind_operations',guide=guide,library=index.reference_designs,
                    **index.native_import_args())
    _publish_in_place(guide,result['guide'])
    return result['reports']


def _bind_production(step,catalog,country,known_variants=None):
    from .backend_core import _request,_publish_in_place
    index=catalog_index(catalog)
    result=_request(0,'import_bind_production',step=step,country=country,library=index.reference_designs,
                    declared=known_variants,**index.native_import_args())
    _publish_in_place(step,result['step'])
    return result['reason']
