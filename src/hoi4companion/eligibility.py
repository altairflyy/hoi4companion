"""Selezione conservativa degli alberi iniziali compatibili con i DLC.

Le condizioni dinamiche non sono indovinate. I pesi vengono confrontati solo
per tag con condizioni statiche risolte; non si pretende di leggere la partita.
"""
from __future__ import annotations

import math
from pathlib import Path
import re

from .clausewitz import Node
from .jomini_parser import parse_file
from .dlc import DlcInventory


def load_scripted_triggers(files: list[Path]) -> tuple[dict[str,Node],list[str]]:
    from .backend_core import _request
    from .script_document import decode
    value=_request(0,'source_scripted_triggers',files=list(map(str,files)))
    return {key:decode(node) for key,node in value['scripts'].items()},value['warnings']


def _and(values: list[bool | None]) -> bool | None:
    return False if False in values else None if None in values else True


def _or(values: list[bool | None]) -> bool | None:
    return True if True in values else None if None in values else False


def _not(value: bool | None) -> bool | None:
    return None if value is None else not value


def government_exclusions(node):
    """Project provably false ideology variants; unknown triggers stay unknown."""
    if not isinstance(node,Node):return []
    governments={'communism','fascism','neutrality','democratic'}
    def gather(block):
        for key,value in block.entries:
            if key=='has_government' and isinstance(value,str):governments.add(value)
            elif isinstance(value,Node):gather(value)
    gather(node)
    def project(block,government):
        entries=[]
        for key,value in block.entries:
            if key=='has_government' and isinstance(value,str):entries.append(('always','yes' if value==government else 'no'))
            elif key.upper() in ('AND','OR','NOT','NAND','NOR') and isinstance(value,Node):entries.append((key.upper(),project(value,government)))
            elif key=='always':entries.append((key,value))
            else:entries.append(('unobserved_runtime_predicate','unknown'))
        return Node(entries)
    return sorted(government for government in governments if condition(project(node,government),'',None,{}) is False)


def excluded_focus_branches(trees, dlcs):
    """Proven DLC-disabled identities, distinct from dynamic availability.

    Evaluate only DLC/always/boolean predicates. Country, game-rule and other
    runtime predicates remain unknown. Descendants are excluded only when an
    entire required prerequisite group is already excluded; an OR alternative,
    missing parent or unknown condition does not establish exclusion.
    """
    if dlcs is None:return []
    def static(node):
        return Node([(key.upper() if key.upper() in ('AND','OR','NOT','NAND','NOR') else
                      key if key in ('has_dlc','always') else 'unobserved_runtime_predicate',
                      static(value) if isinstance(value,Node) else value) for key,value in node.entries])
    excluded=[]
    for scope,tree in trees.items():
        disabled={key:'DLC condition on allow_branch is false.' for key,node in getattr(tree,'branch_conditions',{}).items()
                  if condition(static(node),'',dlcs,{}) is False}
        changed=True
        while changed:
            changed=False
            for key,focus in tree.focuses.items():
                if key not in disabled and any(group and all(parent in disabled for parent in group)
                                               for group in focus.prerequisites):
                    disabled[key]='Every alternative in a required branch is excluded by DLC conditions.'
                    changed=True
        excluded.extend({'scope':scope,'id':key,'reason':reason} for key,reason in sorted(disabled.items()))
    return excluded


def condition(node: Node, tag: str, dlcs: DlcInventory, scripts: dict[str, Node],
              stack: tuple[str, ...] = ()) -> bool | None:
    values = []
    for key, value in node.entries:
        if key in ('AND', 'OR', 'NOT', 'NOR', 'NAND') and isinstance(value, Node):
            if key == 'AND':
                answer = condition(value, tag, dlcs, scripts, stack)
            else:
                parts = [condition(Node([(k, v)]), tag, dlcs, scripts, stack) for k, v in value.entries]
                if key == 'OR':
                    answer = _or(parts)
                elif key == 'NOR':
                    answer = _not(_or(parts))
                elif key == 'NAND':
                    answer = _not(_and(parts))
                else:
                    # Un singolo predicato NOT è non ambiguo. Blocchi multipli
                    # non sono necessari ai selettori attuali: non li indoviniamo.
                    answer = _not(parts[0]) if len(parts) == 1 else None
        elif key == 'has_dlc' and isinstance(value, str):
            answer = dlcs.enabled(value)
        elif key in ('tag', 'original_tag') and isinstance(value, str):
            # Catalogo iniziale: tag corrente e originale coincidono.
            answer = tag == value if re.fullmatch(r'[A-Z0-9]{3}', value) else None
        elif key == 'always' and value in ('yes', 'no'):
            answer = value == 'yes'
        elif key in scripts and value in ('yes', 'no') and key not in stack and len(stack) < 30:
            answer = condition(scripts[key], tag, dlcs, scripts, (*stack, key))
            if value == 'no':
                answer = _not(answer)
        else:
            answer = None
        values.append(answer)
    return _and(values)


def selector_tags(node: Node, scripts: dict[str, Node], stack: tuple[str, ...] = ()) -> set[str]:
    tags = set()
    for key, value in node.entries:
        if key in ('tag', 'original_tag') and isinstance(value, str) and re.fullmatch(r'[A-Z0-9]{3}', value):
            tags.add(value)
        elif isinstance(value, Node):
            tags.update(selector_tags(value, scripts, stack))
        elif key in scripts and key not in stack and len(stack) < 30:
            tags.update(selector_tags(scripts[key], scripts, (*stack, key)))
    return tags


def _number(raw: str) -> float | None:
    try:
        number = float(raw)
        return number if math.isfinite(number) else None
    except (ValueError, TypeError):
        return None


def score(selector: Node, tag: str, dlcs: DlcInventory, scripts: dict[str, Node]) -> float | None:
    value = _number(selector.scalar('factor', selector.scalar('base')))
    if value is None or any(k not in ('factor', 'base', 'modifier') for k, _ in selector.entries):
        return None
    for modifier in selector.blocks('modifier'):
        rules = Node([(k, v) for k, v in modifier.entries if k not in ('add', 'factor')])
        applies = condition(rules, tag, dlcs, scripts)
        if applies is False:
            continue
        if applies is None:
            return None
        add, factor = modifier.scalar('add'), modifier.scalar('factor')
        if bool(add) == bool(factor):
            return None
        change = _number(add or factor)
        if change is None:
            return None
        value = value + change if add else value * change
        if not math.isfinite(value):
            return None
    return value


def filter_trees(trees: dict, dlcs: DlcInventory, scripts: dict[str,Node]) -> tuple[dict,list[dict]]:
    from .backend_core import _request
    from .script_document import encode
    value=_request(0,'source_filter_trees',trees={key:{'id':key,'source':tree.source,'country_selector':encode(tree.country_selector)} for key,tree in trees.items()},
                   dlcs={'items':dlcs.items,'complete':dlcs.complete} if dlcs is not None else None,
                   scripts={key:encode(node) for key,node in scripts.items()})
    visible={key:trees[key] for key in value['visible']}
    for key,tree in visible.items():tree.eligible_countries=value['countries'][key]
    return visible,value['excluded']
