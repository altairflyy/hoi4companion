"""Parser read-only per il formato script Clausewitz/Paradox.

Mantiene l'ordine e le chiavi ripetute: per i focus è essenziale distinguere
`prerequisite = {focus=A focus=B}` (OR) da due blocchi `prerequisite` (AND).
Non interpreta condizioni o esegue script del gioco.
"""
from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(slots=True)
class Node:
    entries: list[tuple[str, str | 'Node']] = field(default_factory=list)
    # Optional typed source fields from Jomini's deserializer. The full ordered
    # document stays authoritative for conditions and fields not migrated yet.
    projection: dict | None = field(default=None, compare=False, repr=False)
    source_encoding: str | None = field(default=None, compare=False, repr=False)

    def all(self, key: str) -> list[str | 'Node']:
        return [value for name, value in self.entries if name == key]

    def first(self, key: str, default=None):
        return next((value for name, value in self.entries if name == key), default)

    def scalar(self, key: str, default: str = '') -> str:
        value = self.first(key)
        return value if isinstance(value, str) else default

    def blocks(self, key: str) -> list['Node']:
        return [value for value in self.all(key) if isinstance(value, Node)]


def parse(source: str) -> Node:
    from .jomini_parser import parse as native_parse
    return native_parse(source)
