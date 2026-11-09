"""Inventario DLC locale: nessuna interrogazione Steam o modifica al gioco."""
from __future__ import annotations

from dataclasses import dataclass, field
import json
from pathlib import Path
import unicodedata

from .jomini_parser import parse_file


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
    from .backend_core import _request
    return DlcInventory(**_request(0,'installation_dlcs',game=str(game),docs=str(docs) if docs is not None else None))
