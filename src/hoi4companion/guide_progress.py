"""In-memory guide progress, separate from definitions and live observations.

Run IDs belong to the companion, not HOI4. A process session is not a campaign
identity: loading another save in the same game process may require an explicit
``create`` call (the UI's New game action). No completion is cached here.
"""
from __future__ import annotations

from copy import deepcopy
from threading import RLock
from uuid import uuid4


def _identifier(value, label, *, optional=False, maximum=None):
    if optional and value is None:
        return
    if (not isinstance(value, str) or not value.strip()
            or maximum is not None and len(value) > maximum):
        raise ValueError(label + ' non valido')


def _checks(value):
    if not isinstance(value, dict) or any(not isinstance(key, str) or type(checked) is not bool
                                          for key, checked in value.items()):
        raise ValueError('Conferme della partita non valide')
    return dict(value)


class RunStore:
    """Thread-safe snapshots of caller-owned runs; no files, game or clock APIs."""

    def __init__(self, id_factory=None):
        self._id_factory = id_factory if id_factory is not None else lambda: uuid4().hex
        self._runs = {}
        self._active = {}
        self._lock = RLock()

    def _new(self, guide_id, session_id, checks, reserved):
        _identifier(guide_id, 'Identità guida')
        _identifier(session_id, 'Sessione', optional=True)
        run_id = self._id_factory()
        _identifier(run_id, 'Identità partita', maximum=160)
        if run_id in reserved:
            raise ValueError('Identità partita duplicata')
        return {'schema_version': 1, 'guide_id': guide_id, 'run_id': run_id,
                'session_id': session_id, 'checks': dict(checks)}

    def ensure(self, guide_id, legacy_checks=None):
        """Create an initial run once, importing only boolean legacy checks."""
        _identifier(guide_id, 'Identità guida')
        with self._lock:
            if guide_id in self._active:
                return self.get(guide_id)
            if legacy_checks is not None and not isinstance(legacy_checks, dict):
                raise ValueError('Conferme legacy non valide')
            checks = {key: value for key, value in (legacy_checks or {}).items()
                      if isinstance(key, str) and type(value) is bool}
            record = self._new(guide_id, None, checks, self._runs)
            self._runs[record['run_id']] = record
            self._active[guide_id] = record['run_id']
            return deepcopy(record)

    def create(self, guide_id, session_id=None):
        """Activate an empty run and retain every previous run for this guide."""
        with self._lock:
            record = self._new(guide_id, session_id, {}, self._runs)
            self._runs[record['run_id']] = record
            self._active[guide_id] = record['run_id']
            return deepcopy(record)

    def select(self, guide_id, run_id):
        """Activate a historical run only when it belongs to the same guide."""
        with self._lock:
            record = self.get(guide_id, run_id)
            self._active[guide_id] = record['run_id']
            return record

    def get(self, guide_id, run_id=None):
        """Return an isolated active/historical snapshot; never create implicitly."""
        _identifier(guide_id, 'Identità guida')
        with self._lock:
            if run_id is None:
                run_id = self._active.get(guide_id)
            record = self._runs.get(run_id) if isinstance(run_id, str) else None
            if record is None or record['guide_id'] != guide_id:
                raise ValueError('Partita della guida non trovata')
            return deepcopy(record)

    def update(self, guide_id, checks):
        """Replace the active run's checklist after validating the whole update."""
        checks = _checks(checks)
        with self._lock:
            record = self.get(guide_id)
            record['checks'] = checks
            self._runs[record['run_id']] = record
            return deepcopy(record)

    def bind(self, session_id):
        """Bind unbound active runs; rotate bound runs on process-session changes.

        This is a consumer safeguard, not proof of a campaign/save identity.
        All replacements are prepared before committing, including ID checks.
        """
        if session_id is None:
            return False
        _identifier(session_id, 'Sessione')
        with self._lock:
            replacements = []
            reserved = set(self._runs)
            for guide_id, run_id in self._active.items():
                current = self._runs[run_id]
                if current['session_id'] == session_id:
                    continue
                if current['session_id'] is None:
                    record = deepcopy(current)
                    record['session_id'] = session_id
                else:
                    record = self._new(guide_id, session_id, {}, reserved)
                    reserved.add(record['run_id'])
                replacements.append(record)
            for record in replacements:
                self._runs[record['run_id']] = record
                self._active[record['guide_id']] = record['run_id']
            return bool(replacements)

    def metadata(self):
        """Return active identities without duplicating checklist data in SSE."""
        with self._lock:
            return {guide_id: {key: value for key, value in self._runs[run_id].items() if key != 'checks'}
                    for guide_id, run_id in self._active.items()}
