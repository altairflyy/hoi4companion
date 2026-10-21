"""Inventario DLC locale: nessuna interrogazione Steam o modifica al gioco."""
from __future__ import annotations

from dataclasses import dataclass, field
import json
from pathlib import Path
import unicodedata

from .clausewitz import parse


def normalise(value: str) -> str:
    value = unicodedata.normalize('NFKD', value)
    return ' '.join(''.join(c for c in value if not unicodedata.combining(c)).casefold().split())


@dataclass
class DlcInventory:
    items: list[dict] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)
    complete: bool = True

    def enabled(self, name: str) -> bool | None:
        matches = [x for x in self.items if normalise(x['name']) == normalise(name)]
        if any(x['status'] in ('enabled', 'integrated') for x in matches):
            return True
        if matches and all(x['status'] in ('disabled', 'missing') for x in matches):
            return False
        return False if self.complete and not matches else None

    def to_json(self) -> dict:
        return {'items': self.items, 'complete': self.complete,
                'enabled_count': sum(x['status'] in ('enabled', 'integrated') for x in self.items),
                'basis': 'File installati e disattivazioni del launcher; non legge la partita.'}


def discover_dlcs(game: Path, docs: Path | None) -> DlcInventory:
    inventory = DlcInventory()
    disabled: set[str] = set()
    launcher_known = True
    config = docs / 'dlc_load.json' if docs else None
    if config is None or not config.is_file():
        launcher_known = False
        inventory.warnings.append('Configurazione DLC del launcher assente: attivazione dei DLC non integrati da verificare.')
    else:
        try:
            data = json.loads(config.read_text(encoding='utf-8-sig'))
            entries = data['disabled_dlcs']
            if not isinstance(entries, list) or not all(isinstance(x, str) for x in entries):
                raise ValueError('disabled_dlcs non è una lista di stringhe')
            disabled = {x.replace('\\', '/').casefold() for x in entries}
        except (OSError, ValueError, KeyError, TypeError) as exc:
            launcher_known = False
            inventory.warnings.append(f'Configurazione DLC del launcher non leggibile: {exc}')
    inventory.complete = launcher_known
    for group in ('dlc', 'integrated_dlc'):
        folder = game / group
        if not folder.is_dir():
            continue
        try:
            descriptors = sorted(folder.rglob('*.dlc'))
        except OSError as exc:
            inventory.complete = False
            inventory.warnings.append(f'Inventario DLC incompleto: {exc}')
            continue
        for descriptor in descriptors:
            try:
                node = parse(descriptor.read_text(encoding='utf-8-sig'))
                name = node.scalar('name')
                if not name:
                    raise ValueError('nome DLC assente')
                rel = descriptor.relative_to(game).as_posix()
                identifiers = {rel.casefold(), descriptor.name.casefold(), descriptor.stem.casefold(),
                               name.casefold(), node.scalar('steam_id').casefold(), node.scalar('pops_id').casefold()}
                integrated = group == 'integrated_dlc'
                payload = node.scalar('archive') or node.scalar('path')
                # I DLC integrati conservano spesso un vecchio path="dlc/...".
                payload_path = game / payload.replace('\\', '/') if payload else None
                present = (descriptor.parent.is_dir() if integrated else
                           bool(payload_path and (payload_path.is_file() if node.scalar('archive') else payload_path.is_dir())))
                if not present:
                    status = 'missing'
                elif integrated:
                    status = 'integrated'
                elif identifiers & disabled:
                    status = 'disabled'
                else:
                    status = 'enabled' if launcher_known else 'unknown'
                inventory.items.append({'name': name, 'status': status, 'descriptor': rel,
                                        'steam_id': node.scalar('steam_id')})
            except (OSError, ValueError, UnicodeError) as exc:
                inventory.complete = False
                inventory.warnings.append(f'DLC non leggibile ({descriptor.name}): {exc}')
    return inventory
