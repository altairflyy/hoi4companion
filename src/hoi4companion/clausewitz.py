"""Parser read-only per il formato script Clausewitz/Paradox.

Mantiene l'ordine e le chiavi ripetute: per i focus è essenziale distinguere
`prerequisite = {focus=A focus=B}` (OR) da due blocchi `prerequisite` (AND).
Non interpreta condizioni o esegue script del gioco.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterator


@dataclass(slots=True)
class Node:
    entries: list[tuple[str, str | 'Node']] = field(default_factory=list)

    def all(self, key: str) -> list[str | 'Node']:
        return [value for name, value in self.entries if name == key]

    def first(self, key: str, default=None):
        return next((value for name, value in self.entries if name == key), default)

    def scalar(self, key: str, default: str = '') -> str:
        value = self.first(key)
        return value if isinstance(value, str) else default

    def blocks(self, key: str) -> list['Node']:
        return [value for value in self.all(key) if isinstance(value, Node)]


def tokenize(source: str) -> Iterator[str]:
    """Stream di token; rimuove commenti fuori dalle stringhe."""
    i, end = 0, len(source)
    while i < end:
        char = source[i]
        if char.isspace() or char == '\ufeff':
            i += 1
            continue
        if char == '#':
            while i < end and source[i] not in '\r\n':
                i += 1
            continue
        if char in '{}':
            i += 1
            yield char
            continue
        if char in '=<>!?':
            nxt = source[i:i+2]
            if nxt in ('<=', '>=', '!=', '?='):
                yield nxt
                i += 2
            else:
                yield char
                i += 1
            continue
        if char == '"':
            i += 1
            chars: list[str] = []
            while i < end:
                c = source[i]
                if c == '"':
                    i += 1
                    break
                if c == '\\' and i + 1 < end and source[i+1] in ('"', '\\'):
                    chars.append(source[i+1])
                    i += 2
                    continue
                chars.append(c)
                i += 1
            else:
                raise ValueError('Stringa tra virgolette non terminata')
            yield ''.join(chars)
            continue
        start = i
        while i < end and not source[i].isspace() and source[i] not in '{}#=<>!?"':
            i += 1
        if i == start:
            raise ValueError(f'Carattere imprevisto in posizione {i}')
        yield source[start:i]


class _Tokens:
    def __init__(self, tokens: Iterator[str]):
        self.iterator = iter(tokens)
        self.buffer: str | None = None

    def peek(self) -> str | None:
        if self.buffer is None:
            self.buffer = next(self.iterator, None)
        return self.buffer

    def get(self) -> str | None:
        value = self.peek()
        self.buffer = None
        return value


def _parse_block(tokens: _Tokens, nested: bool, depth: int) -> Node:
    if depth > 180:
        raise ValueError('Profondità eccessiva nello script')
    result = Node()
    while True:
        key = tokens.get()
        if key is None:
            if nested:
                raise ValueError('Blocco aperto non chiuso')
            return result
        if key == '}':
            if not nested:
                raise ValueError('Parentesi graffa chiusa senza apertura')
            return result
        if key == '{':
            # Blocchi anonimi (rari) mantenuti senza inventare una chiave.
            result.entries.append(('', _parse_block(tokens, True, depth+1)))
            continue
        op = tokens.peek()
        if op in ('=', '<', '>', '<=', '>=', '!=', '?='):
            tokens.get()
            value = tokens.get()
            if value is None:
                raise ValueError(f'Valore mancante per {key}')
            if value == '{':
                parsed: str | Node = _parse_block(tokens, True, depth+1)
            elif value == '}':
                raise ValueError(f'Valore mancante per {key}')
            else:
                parsed = value
            # L'operatore rimane significativo negli script; per i campi
            # dei focus usiamo solo '='; negli altri conserviamo '<', ecc.
            result.entries.append((key if op == '=' else f'{key} {op}', parsed))
        else:
            result.entries.append(('', key))


def parse(source: str) -> Node:
    return _parse_block(_Tokens(tokenize(source)), False, 0)
