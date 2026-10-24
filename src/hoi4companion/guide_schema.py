"""Convalida delle estensioni additive Guide Schema v1.

Le definizioni della guida sono dati: nessuna condizione viene eseguita,
nessuno stato del gioco viene acquisito o modificato.
"""
from __future__ import annotations

from collections import deque
from datetime import date
import re
from urllib.parse import urlsplit

from .contracts import is_json_number, validate_condition


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
    if value is None or type(value) in (str, bool):
        if type(value) is str and len(value) > 4096:
            raise ValueError('Parametro testuale troppo lungo')
        return
    if type(value) in (int, float):
        if not is_json_number(value):
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


def _valid_url(value):
    if not isinstance(value, str) or not value.startswith(('https://', 'http://')) or any(c.isspace() or ord(c) < 32 for c in value):
        return False
    try:
        parsed = urlsplit(value)
        return bool(parsed.hostname) and (parsed.port is None or 0 <= parsed.port <= 65535)
    except ValueError:
        return False


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
        if 'url' in source and not _valid_url(source['url']):
            raise ValueError('URL fonte non valido')
    for phase in guide.get('phases', []):
        if not isinstance(phase.get('title'), str) or not phase['title']:
            raise ValueError('Titolo fase mancante')
        if 'when' in phase:
            validate_condition(phase['when'])

    rows = [step for steps in guide['tracks'].values() for step in steps]
    keys = {step['key'] for step in rows}
    reminders = {step['key'] for step in rows if step.get('tracking') == 'reminder'}
    pending = {key: 0 for key in keys}
    successors = {key: [] for key in keys}

    for step in rows:
        key = step['key']
        if step.get('tracking', 'action') not in ('action', 'reminder'):
            raise ValueError(f'Modalità monitoraggio non valida: {key}')
        if step.get('tracking') == 'reminder' and (step['action']['kind'] != 'manual'
                or 'completion' in step or 'started_when' in step):
            raise ValueError(f'Promemoria incompatibile con azione o completamento: {key}')
        deps = step.get('depends_on', [])
        if not isinstance(deps, list) or len(deps) > 64 or any(
                not isinstance(dep, str) or dep not in keys or dep == key for dep in deps
        ) or len(deps) != len(set(deps)):
            raise ValueError(f'Dipendenze non valide: {key}')
        if any(dep in reminders for dep in deps):
            raise ValueError(f'Dipendenza da promemoria non completabile: {key}')
        pending[key] = len(deps)
        for dep in deps:
            successors[dep].append(key)

        if 'phase' in step and (not isinstance(step['phase'], str) or step['phase'] not in phases):
            raise ValueError(f'Fase sconosciuta: {key}')
        if 'completion' in step:
            validate_condition(step['completion'])
        if 'started_when' in step:
            if step['action']['kind'] not in ('manual', 'special_project'):
                raise ValueError(f'Condizione di avvio incompatibile con azione: {key}')
            validate_condition(step['started_when'])
        if 'params' in step['action']:
            params = step['action']['params']
            if not isinstance(params, dict):
                raise ValueError(f'Parametri azione non validi: {key}')
            _json_data(params)
            if 'game_date' in params:
                value=params['game_date']
                if not isinstance(value,str) or not re.fullmatch(r'\d{1,4}\.\d{1,2}\.\d{1,2}',value):
                    raise ValueError(f'Data della partita non valida: {key}')
                try:date(*map(int,value.split('.')))
                except ValueError:raise ValueError(f'Data della partita non valida: {key}') from None

        evidence = step.get('evidence', [])
        if not isinstance(evidence, list) or len(evidence) > 32:
            raise ValueError(f'Evidenze non valide: {key}')
        for item in evidence:
            if (not isinstance(item, dict) or not isinstance(item.get('source_id'), str) or item['source_id'] not in sources or
                    item.get('basis') not in ('explicit', 'visual', 'inferred', 'game_derived', 'unverified')):
                raise ValueError(f'Provenienza non valida: {key}')
            for field in ('timestamp_seconds', 'end_seconds'):
                if field in item and (not is_json_number(item[field]) or item[field] < 0):
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
