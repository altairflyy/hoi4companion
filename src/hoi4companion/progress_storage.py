"""Atomic local progress snapshots. No game data or credentials are stored."""
import json
from pathlib import Path
import sqlite3
from contextlib import contextmanager


class ProgressStorageError(ValueError):
    pass


class ProgressDatabase:
    def __init__(self,path):
        self.path=Path(path)
        self.revision=0
        try:
            self.path.parent.mkdir(parents=True,exist_ok=True)
            with self.connect() as db:
                db.execute('CREATE TABLE IF NOT EXISTS progress (id INTEGER PRIMARY KEY CHECK(id=1), version INTEGER NOT NULL, revision INTEGER NOT NULL, payload TEXT NOT NULL)')
        except (sqlite3.Error,OSError) as exc:
            raise ProgressStorageError('Archivio avanzamento non leggibile; file originale conservato: '+str(exc)) from exc

    @contextmanager
    def connect(self):
        db=sqlite3.connect(self.path,timeout=3)
        try:
            db.execute('PRAGMA synchronous=FULL')
            with db:yield db
        finally:db.close()

    def load(self):
        try:
            with self.connect() as db:row=db.execute('SELECT version,revision,payload FROM progress WHERE id=1').fetchone()
            if row is None:return None
            version,revision,payload=row
            if version!=1 or type(revision) is not int or revision<1 or len(payload)>16*1024*1024:
                raise ValueError('Versione o dimensione archivio non supportata')
            value=json.loads(payload)
            self.revision=revision
            return value
        except (sqlite3.Error,ValueError) as exc:
            raise ProgressStorageError('Archivio avanzamento non valido; file originale conservato: '+str(exc)) from exc

    def save(self,value):
        payload=json.dumps(value,ensure_ascii=False,allow_nan=False,separators=(',',':'))
        if len(payload)>16*1024*1024:raise ProgressStorageError('Archivio avanzamento troppo grande')
        try:
            with self.connect() as db:
                db.execute('BEGIN IMMEDIATE')
                row=db.execute('SELECT revision FROM progress WHERE id=1').fetchone()
                actual=row[0] if row else 0
                if actual!=self.revision:
                    raise ProgressStorageError('Avanzamento modificato da un’altra istanza: riavvia Companion prima di salvare')
                revision=self.revision+1
                db.execute('INSERT INTO progress VALUES(1,1,?,?) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,payload=excluded.payload',(revision,payload))
            self.revision=revision
        except sqlite3.Error as exc:
            raise ProgressStorageError('Avanzamento non salvato: '+str(exc)) from exc
