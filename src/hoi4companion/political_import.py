"""Resolve declared political identities from installed scripts, never run triggers.

English is the reference; local labels and trait names are aliases. A catalog
association proves identity, not availability or completion in a live campaign.
"""
from .research_upgrades import normal_name
from .resolution import catalog_index, ResolutionContext


def political_catalog(game, mods, labels, reference):
    from .backend_core import _request,_publish_in_place
    value=_request(0,'source_political',game=str(game),mods=list(map(str,mods)),labels=labels,reference=reference)
    return value['rows'],value['warnings']


def _fact(key,value):
    return {'op':'fact','key':key,'test':'contains','value':value}


def bind_political(step,catalog,country,audit=None):
    from .backend_core import _request,_publish_in_place
    index=catalog_index(catalog)
    result=_request(0,'import_bind_political',step=step,country=country,audit=audit is not None,**index.native_import_args())
    _publish_in_place(step,result['step'])
    if audit is not None:audit.extend(result['audit'])
    return result['reason']
