"""Import snapshot espliciti e supporto sperimentale per save testuali HOI4.

Mai inferire i focus da testo non strutturato: in caso di ambiguità lo stato
viene segnalato come sconosciuto.
"""
from __future__ import annotations

from dataclasses import dataclass, asdict
import json
from pathlib import Path
from .clausewitz import Node, parse


@dataclass(slots=True)
class Snapshot:
    tag: str
    completed: list[str]
    current: str | None = None
    progress: float | None = None
    source: str = 'manuale'
    note: str = ''

    def to_json(self) -> dict:
        return asdict(self)


def _extract_focus_country(country: Node, tag: str) -> Snapshot:
    completed: list[str] = []
    for key in ('completed_focuses', 'completed_focus'):
        for value in country.all(key):
            if isinstance(value, Node):
                # Salvataggi Clausewitz: elenco di valori o chiavi = yes.
                for name, item in value.entries:
                    if name == '' and isinstance(item, str):
                        completed.append(item)
                    elif name and isinstance(item, str) and item in ('yes', '1'):
                        completed.append(name)
            elif isinstance(value, str):
                completed.append(value)
    focus_node = country.first('focus')
    current = country.scalar('national_focus') or country.scalar('current_focus') or None
    progress = None
    if isinstance(focus_node, Node):
        current = current or focus_node.scalar('current') or None
        for key in ('progress', 'amount'):
            raw = focus_node.scalar(key)
            if raw:
                try:
                    progress = float(raw)
                except ValueError:
                    pass
                break
    if current in ('0', 'none', '---'):
        current = None
    return Snapshot(tag=tag, completed=list(dict.fromkeys(completed)), current=current,
                    progress=progress, source='save_testuale',
                    note='Lettura sperimentale: verificare lo stato in gioco. Il progresso non è normalizzato.')


def read_snapshot(path: Path, tag: str = 'AUS') -> Snapshot:
    if not path.is_file():
        raise ValueError('File stato non trovato')
    if path.stat().st_size > 80_000_000:
        raise ValueError('File oltre 80 MB: importazione bloccata per evitare rallentamenti')
    if path.suffix.lower() == '.json':
        data = json.loads(path.read_text(encoding='utf-8-sig'))
        if not isinstance(data, dict):
            raise ValueError('Snapshot JSON non valido')
        completed = data.get('completed_focuses', [])
        if not isinstance(completed, list) or not all(isinstance(x, str) for x in completed):
            raise ValueError('completed_focuses deve essere una lista di ID')
        current = data.get('current_focus')
        if current is not None and not isinstance(current, str):
            raise ValueError('current_focus non valido')
        progress = data.get('focus_progress')
        if progress is not None and (isinstance(progress, bool) or not isinstance(progress, (float, int))):
            raise ValueError('focus_progress deve essere un numero')
        return Snapshot(tag=str(data.get('tag', tag)), completed=list(dict.fromkeys(completed)),
                        current=current, progress=progress, source='snapshot_json',
                        note='Snapshot fornito manualmente, non sincronizzato con HOI4.')
    if path.suffix.lower() != '.hoi4':
        raise ValueError('Sono accettati soltanto .json oppure salvataggi .hoi4 testuali')
    raw = path.read_bytes()
    if raw.startswith(b'HOI4bin') or raw.startswith(b'PK\x03\x04'):
        raise ValueError('Salvataggio binario/compresso non supportato: non viene interpretato')
    if not raw.startswith(b'HOI4txt'):
        raise ValueError('Formato di salvataggio non riconosciuto come HOI4 testuale')
    try:
        content = raw[7:].decode('utf-8-sig')
        root = parse(content)
    except (UnicodeError, ValueError, RecursionError) as exc:
        raise ValueError(f'Impossibile leggere il salvataggio: {exc}') from exc
    countries = root.first('countries')
    if not isinstance(countries, Node):
        raise ValueError('Blocco countries assente nel salvataggio')
    country = countries.first(tag)
    if not isinstance(country, Node):
        raise ValueError(f'Nazione {tag} non presente nel salvataggio')
    snap = _extract_focus_country(country, tag)
    # Se non troviamo i campi, non dichiariamo che il giocatore abbia zero focus.
    if not snap.completed and not snap.current:
        raise ValueError('Campi focus non riconosciuti: nessuno stato dedotto dal salvataggio')
    return snap
