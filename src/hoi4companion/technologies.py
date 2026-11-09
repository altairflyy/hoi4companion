"""Catalogo ricerca dai file locali: nessuna simulazione della ricercabilità."""
from __future__ import annotations

import math
from pathlib import Path
from .clausewitz import Node
from .jomini_parser import parse_file
from .eligibility import condition
from .tooltips import localise


def _normal(node):
    return Node([(k.upper() if k.upper() in ('AND', 'OR', 'NOT', 'NAND', 'NOR') else k,
                  _normal(v) if isinstance(v, Node) else v) for k, v in node.entries])


def _values(node):
    return [v for _, v in node.entries if isinstance(v, str)] if isinstance(node, Node) else []


def _number(raw, variables):
    seen = set()
    while raw.startswith('@') and raw not in seen:
        seen.add(raw)
        raw = variables.get(raw, '')
    try:
        value = float(raw)
        return value if math.isfinite(value) else None
    except (TypeError, ValueError):
        return None


def load_technologies(files,tag_files,labels,dlcs,scripts,country='AUS',gui_files=(),*,definition_metadata=None):
    from .backend_core import _request
    from .script_document import encode,decode
    value=_request(0,'technology_sources',files=list(map(str,files)),tag_files=list(map(str,tag_files)),labels=labels,
                   dlcs={'items':dlcs.items,'complete':dlcs.complete} if dlcs is not None else None,
                   scripts={key:encode(node) for key,node in scripts.items()},country=country,definitions=definition_metadata is not None,gui_files=list(map(str,gui_files)))
    if definition_metadata is not None:
        definition_metadata.update({section:{key:decode(node) for key,node in values.items()} for section,values in value['definitions'].items()})
    catalog=value['catalog']
    return catalog
