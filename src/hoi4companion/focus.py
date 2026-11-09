"""Importazione e valutazione conservativa degli alberi dei focus."""
from __future__ import annotations

from dataclasses import dataclass, field, asdict
from pathlib import Path
from .clausewitz import Node
from .jomini_parser import parse_file
from .tooltips import localise, focus_tooltips


def _refs(block: Node, field_name: str) -> list[str]:
    return [value for value in block.all(field_name) if isinstance(value, str)]


def _num(source: str, default: float) -> float:
    try:
        return float(source)
    except (TypeError, ValueError):
        return default


@dataclass
class Focus:
    id: str
    name: str
    description: str
    icon: str
    x: float
    y: float
    relative_to: str
    cost: float
    prerequisites: list[list[str]]
    mutually_exclusive: list[str]
    has_conditions: bool
    has_bypass: bool
    filters: list[str]
    source: str
    position_warning: bool = False
    icon_variants: list[str] = field(default_factory=list)
    tooltip_sections: list[dict] = field(default_factory=list)
    localization_key: str = ''
    bypass_possible: bool | None = None
    sequence_effects_safe: bool | None = None
    external_completion_possible: bool | None = None
    repeatable: bool = False
    criteria: dict | None = None

    def to_json(self) -> dict:
        value = asdict(self)
        value['estimated_days'] = round(self.cost * 7)
        return value


@dataclass
class Tree:
    id: str
    source: str
    focuses: dict[str, Focus] = field(default_factory=dict)
    shared: list[str] = field(default_factory=list)
    country_selector: Node | None = None
    default: bool = False
    eligible_countries: list[str] = field(default_factory=list)
    branch_conditions: dict[str, Node] = field(default_factory=dict, repr=False)

    def to_json(self) -> dict:
        return {'id': self.id, 'source': self.source, 'count': len(self.focuses),
                'focuses': [f.to_json() for f in self.focuses.values()]}


def _build_focus(node: Node, labels: dict[str, str], source: str, *, jointly_completed=False) -> Focus | None:
    from .backend_core import _request
    from .script_document import encode
    value=_request(0,'focus_build',node=encode(node),labels=labels,source=source,joint=jointly_completed)
    return Focus(**value) if value is not None else None




def load_trees(files: list[Path], labels: dict[str,str]) -> tuple[dict[str,Tree],list[str]]:
    from .backend_core import _request
    from .script_document import decode
    value=_request(0,'focus_trees',files=list(map(str,files)),labels=labels)
    trees={}
    for identity,item in value['trees'].items():
        item={**item,'focuses':{key:Focus(**focus) for key,focus in item['focuses'].items()},
              'country_selector':decode(item['country_selector']),
              'branch_conditions':{key:decode(node) for key,node in item['branch_conditions'].items()}}
        trees[identity]=Tree(**item)
    return trees,value['warnings']


def load_continuous_focuses(files,labels):
    from .backend_core import _request
    value=_request(0,'focus_continuous',files=list(map(str,files)),labels=labels)
    return {key:Focus(**focus) for key,focus in value['focuses'].items()},value['warnings']


def evaluate(tree: Tree, completed: set[str], current: str | None = None) -> dict[str, str]:
    """Non prova a valutare i trigger complessi: stato 'da verificare'."""
    result = {}
    for fid, focus in tree.focuses.items():
        if fid in completed:
            result[fid] = 'completed'
        elif fid == current:
            result[fid] = 'active'
        elif any(x in completed for x in focus.mutually_exclusive):
            result[fid] = 'excluded'
        elif not all(any(x in completed for x in group) for group in focus.prerequisites):
            result[fid] = 'locked'
        elif focus.has_conditions:
            result[fid] = 'verify'
        else:
            result[fid] = 'ready'
    return result


def prerequisite_closure(tree: Tree, target: str) -> set[str]:
    """Tutti i *possibili* antenati (con OR non sceglie arbitrariamente un ramo)."""
    result: set[str] = set()
    stack = [target]
    while stack:
        fid = stack.pop()
        if fid in result or fid not in tree.focuses:
            continue
        result.add(fid)
        for group in tree.focuses[fid].prerequisites:
            stack.extend(group)
    return result
