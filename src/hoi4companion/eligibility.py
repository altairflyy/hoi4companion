"""Selezione conservativa degli alberi iniziali compatibili con i DLC.

Le condizioni dinamiche non sono indovinate. I pesi vengono confrontati solo
per tag con condizioni statiche risolte; non si pretende di leggere la partita.
"""
from __future__ import annotations

import math
from pathlib import Path
import re

from .clausewitz import Node, parse
from .dlc import DlcInventory


def load_scripted_triggers(files: list[Path]) -> tuple[dict[str, Node], list[str]]:
    result: dict[str, Node] = {}
    warnings = []
    for file in files:
        try:
            node = parse(file.read_text(encoding='utf-8-sig'))
            result.update((k, v) for k, v in node.entries if isinstance(v, Node))
        except (OSError, ValueError, UnicodeError, RecursionError) as exc:
            warnings.append(f'Trigger non leggibili ({file.name}): {exc}')
    return result, warnings


def _and(values: list[bool | None]) -> bool | None:
    return False if False in values else None if None in values else True


def _or(values: list[bool | None]) -> bool | None:
    return True if True in values else None if None in values else False


def _not(value: bool | None) -> bool | None:
    return None if value is None else not value


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


def filter_trees(trees: dict, dlcs: DlcInventory, scripts: dict[str, Node]) -> tuple[dict, list[dict]]:
    scores: dict[str, dict[str, float | None]] = {}
    tags = {tag for tree in trees.values() if isinstance(tree.country_selector, Node)
            for tag in selector_tags(tree.country_selector, scripts)} | {''}
    for tree in trees.values():
        if isinstance(tree.country_selector, Node):
            scores[tree.id] = {tag: score(tree.country_selector, tag, dlcs, scripts) for tag in tags}
    winners: dict[str, set[str]] = {}
    uncertain_tags = set()
    for tag in {tag for values in scores.values() for tag in values}:
        candidates = {key: values[tag] for key, values in scores.items() if tag in values}
        if any(value is None for value in candidates.values()):
            uncertain_tags.add(tag)
            continue
        best = max(candidates.values(), default=0)
        selected = {key for key, value in candidates.items() if value == best and value > 0}
        # Parità: non scegliere arbitrariamente quale albero il gioco assegnerebbe.
        if len(selected) == 1:
            winners[tag] = selected
    visible, excluded = {}, []
    for tree in trees.values():
        values = scores.get(tree.id, {})
        countries = sorted(tag for tag, selected in winners.items() if tree.id in selected and tag)
        chosen = any(tree.id in selected for selected in winners.values())
        if chosen:
            tree.eligible_countries = countries
            visible[tree.id] = tree
            continue
        if (not values or any(value is None for value in values.values()) or
                any(tag in uncertain_tags and value > 0 for tag, value in values.items() if value is not None)):
            reason = 'Condizioni della partita o attivazione DLC non verificabili dai file locali.'
            category = 'unknown'
        elif any(value > 0 for value in values.values()):
            reason = 'Albero alternativo: priorità inferiore o parità con un altro albero per gli stessi paesi.'
            category = 'alternative'
        else:
            reason = 'Requisiti DLC non soddisfatti, oppure assegnazione solo tramite eventi della partita.'
            category = 'unavailable'
        excluded.append({'id': tree.id, 'source': tree.source, 'reason': reason, 'category': category})
    return visible, excluded
