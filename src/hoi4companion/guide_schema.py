"""Convalida delle estensioni additive Guide Schema v1.

Le definizioni della guida sono dati: nessuna condizione viene eseguita,
nessuno stato del gioco viene acquisito o modificato.
"""
from __future__ import annotations

from collections import deque
import math

from .contracts import validate_condition


def _identifiers(items, label, limit):
    if not isinstance(items, list) or len(items) > limit:
        raise ValueError(f'{label}: elenco non valido')
    seen = set()
    for item in items:
        if not isinstance(item, dict):
            raise ValueError(f'{label}: elemento non valido')
        identifier = item.get('id')
        if not isinstance(identifier, str) or not 0 < len(identifier) <= 160 or identifier in seen:
            raise ValueError(f'{label}: identificatore mancante o duplicato')
        seen.add(identifier)
    return seen


def _json_data(value, depth=0):
    """Limiti ai parametri descrittivi; mai interpretarli come codice."""
    if depth > 8:
        raise ValueError('Parametri azione troppo profondi')
    if value is None or type(value) in (str, bool, int):
        if type(value) is str and len(value) > 4096:
            raise ValueError('Parametro testuale troppo lungo')
        return
    if type(value) is float:
        if not math.isfinite(value):
            raise ValueError('Parametro numerico non finito')
        return
    if isinstance(value, list) and len(value) <= 512:
        for item in value:
            _json_data(item, depth + 1)
        return
    if isinstance(value, dict) and len(value) <= 128:
        for key, item in value.items():
            if not isinstance(key, str) or not 0 < len(key) <= 128:
                raise ValueError('Nome parametro non valido')
            _json_data(item, depth + 1)
        return
    raise ValueError('Parametri azione non validi')


def validate_guide_extensions(guide: dict) -> None:
    """Convalida riferimenti e DAG inter-traccia; compatibile con guide legacy.

    'completion', 'depends_on' ed 'evidence' sono dichiarazioni: non abilitano
    da soli valutazione live, completamenti o nuovi tipi di azione.
    """
    sources = _identifiers(guide.get('sources', []), 'Fonti', 32)
    phases = _identifiers(guide.get('phases', []), 'Fasi', 128)

    for source in guide.get('sources', []):
        if source.get('type') not in ('youtube', 'web', 'file', 'manual'):
            raise ValueError('Tipo fonte non valido')
        if 'url' in source and (not isinstance(source['url'], str) or
                                not source['url'].startswith(('https://', 'http://'))):
            raise ValueError('URL fonte non valido')
    for phase in guide.get('phases', []):
        if not isinstance(phase.get('title'), str) or not phase['title']:
            raise ValueError('Titolo fase mancante')
        if 'when' in phase:
            validate_condition(phase['when'])

    rows = [step for steps in guide['tracks'].values() for step in steps]
    keys = {step['key'] for step in rows}
    pending = {key: 0 for key in keys}
    successors = {key: [] for key in keys}

    for step in rows:
        key = step['key']
        deps = step.get('depends_on', [])
        if not isinstance(deps, list) or len(deps) > 64 or any(
                not isinstance(dep, str) or dep not in keys or dep == key for dep in deps
        ) or len(deps) != len(set(deps)):
            raise ValueError(f'Dipendenze non valide: {key}')
        pending[key] = len(deps)
        for dep in deps:
            successors[dep].append(key)

        if 'phase' in step and (not isinstance(step['phase'], str) or step['phase'] not in phases):
            raise ValueError(f'Fase sconosciuta: {key}')
        if 'completion' in step:
            validate_condition(step['completion'])
        params = step['action'].get('params')
        if params is not None:
            if not isinstance(params, dict):
                raise ValueError(f'Parametri azione non validi: {key}')
            _json_data(params)

        evidence = step.get('evidence', [])
        if not isinstance(evidence, list) or len(evidence) > 32:
            raise ValueError(f'Evidenze non valide: {key}')
        for item in evidence:
            if (not isinstance(item, dict) or item.get('source_id') not in sources or
                    item.get('basis') not in ('explicit', 'visual', 'inferred', 'game_derived', 'unverified')):
                raise ValueError(f'Provenienza non valida: {key}')
            for field in ('timestamp_seconds', 'end_seconds'):
                if field in item and (type(item[field]) not in (int, float) or
                                      not math.isfinite(item[field]) or item[field] < 0):
                    raise ValueError(f'Timestamp non valido: {key}')
            if ('timestamp_seconds' in item and 'end_seconds' in item and
                    item['end_seconds'] < item['timestamp_seconds']):
                raise ValueError(f'Intervallo fonte non valido: {key}')

    queue = deque(key for key, count in pending.items() if count == 0)
    visited = 0
    while queue:
        key = queue.popleft()
        visited += 1
        for other in successors[key]:
            pending[other] -= 1
            if pending[other] == 0:
                queue.append(other)
    if visited != len(keys):
        raise ValueError('Dipendenze della guida cicliche')
