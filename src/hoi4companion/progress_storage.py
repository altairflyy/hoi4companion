"""Native atomic local progress snapshots; existing database format preserved."""
import json
from pathlib import Path
from .storage_core import request


class ProgressStorageError(ValueError):
    pass


class ProgressDatabase:
    def __init__(self,path):
        self.path=Path(path)
        self.revision=0
        try:request('init',self.path)
        except (ValueError,OSError) as exc:
            raise ProgressStorageError('Archivio avanzamento non leggibile; file originale conservato: '+str(exc)) from exc

    def load(self):
        try:
            row=request('load',self.path)
            if row is None:return None
            value=json.loads(row['payload'])
            self.revision=row['revision']
            return value
        except (ValueError,OSError) as exc:
            raise ProgressStorageError('Archivio avanzamento non valido; file originale conservato: '+str(exc)) from exc

    def save(self,value):
        payload=json.dumps(value,ensure_ascii=False,allow_nan=False,separators=(',',':'))
        if len(payload)>16*1024*1024:raise ProgressStorageError('Archivio avanzamento troppo grande')
        try:
            row=request('save',self.path,expected_revision=self.revision,payload=payload)
            self.revision=row['revision']
        except (ValueError,OSError) as exc:
            if 'revision conflict' in str(exc):
                raise ProgressStorageError('Avanzamento modificato da un’altra istanza: riavvia Companion prima di salvare') from exc
            raise ProgressStorageError('Avanzamento non salvato: '+str(exc)) from exc
