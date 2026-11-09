"""Installed doctrine identities and contextual guide commands; no native reads."""
import re
from .resolution import ResolutionContext
from .research_upgrades import guide_language,normal_name
from .contracts import EXPERIENCE_CURRENCIES


def doctrine_catalog(game, mods, labels, reference):
    from .backend_core import _request,_publish_in_place
    value=_request(0,'source_doctrines',game=str(game),mods=list(map(str,mods)),labels=labels,reference=reference)
    return value['rows'],value['warnings']


def doctrine_command(text,index,country):
    from .backend_core import _request
    from .resolution import catalog_index
    index=catalog_index(index)
    return _request(0,'import_doctrine_command',text=text,country=country,**index.native_import_args())


def bind_doctrine(step,index,country):
    from .backend_core import _request,_publish_in_place
    from .resolution import catalog_index
    index=catalog_index(index)
    result=_request(0,'import_bind_doctrine',step=step,country=country,**index.native_import_args())
    _publish_in_place(step,result['step'])
    return result['reason']
