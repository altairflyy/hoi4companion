"""Campaign-scoped progress; optional transactional persistence, no game I/O."""
from __future__ import annotations

from copy import deepcopy
from threading import RLock
from uuid import uuid4
from functools import wraps
import json
import re
from .construction_progress import relative_spec, observed_count
from .construction_progress import validate_baselines
from .campaign import valid_id
from .contracts import normalise_state
from .guide_runtime import evaluate
from .production_progress import relative_spec as production_spec


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


def _mutation(method):
    @wraps(method)
    def transaction(self,*args,**kwargs):
        with self._lock:
            before=self.snapshot()
            try:
                result=method(self,*args,**kwargs)
                if self.snapshot()!=before:
                    for key,row in self._runs.items():
                        old=before['runs'].get(key)
                        if old!=row:
                            row['run_revision']=old.get('run_revision',0)+1 if old else 0
                            if method.__name__ in ('update','observe','create','select') and self._binding and (row.get('campaign_id'),row.get('country'),row.get('session_id'))==self._binding:
                                row['last_tick']=self._tick
                    if self._database:self._database.save(self.snapshot())
                if isinstance(result,dict) and result.get('run_id') in self._runs:
                    return deepcopy(self._runs[result['run_id']])
                return result
            except Exception:
                self._runs,self._active,self._campaign_active=before['runs'],before['active'],before['campaign_active']
                raise
    return transaction


class RunStore:
    """Thread-safe runs. Only the optional storage adapter accesses the disk."""

    def __init__(self, id_factory=None, *, path=None):
        self._id_factory = id_factory if id_factory is not None else lambda: uuid4().hex
        self._runs = {}
        self._active = {}
        self._lock = RLock()
        self._campaign_active={};self._binding=None;self._tick=None;self._database=None
        self._restored_ids=set()
        if path is not None:
            from .progress_storage import ProgressDatabase
            self._database=ProgressDatabase(path)
            value=self._database.load()
            if value is not None:self._load(value)

    def snapshot(self):
        with self._lock:
            return deepcopy({'schema_version':1,'runs':self._runs,'active':self._active,'campaign_active':self._campaign_active})

    def _load(self,value):
        if not isinstance(value,dict) or set(value)!={'schema_version','runs','active','campaign_active'} or type(value['schema_version']) is not int or value['schema_version']!=1:
            raise ValueError('Formato archivio avanzamento non supportato')
        runs,active,campaigns=value['runs'],value['active'],value['campaign_active']
        if any(not isinstance(v,dict) for v in (runs,active,campaigns)) or len(runs)>10000:
            raise ValueError('Archivio avanzamento non valido')
        for key,row in runs.items():
            if not isinstance(row,dict) or type(row.get('schema_version')) is not int or row.get('schema_version')!=1 or row.get('run_id')!=key:raise ValueError('Identità avanzamento non valida')
            _identifier(key,'Partita',maximum=160);_identifier(row.get('guide_id'),'Guida',maximum=160)
            _identifier(row.get('session_id'),'Sessione',optional=True)
            _checks(row.get('checks'));_checks(row.get('started'));validate_baselines(row.get('baselines',{}))
            if type(row.get('run_revision',0)) is not int or row.get('run_revision',0)<0:raise ValueError('Revisione avanzamento non valida')
            if row.get('campaign_id') is not None:
                if not valid_id(row['campaign_id']) or not re.fullmatch('[A-Z0-9]{3}',str(row.get('country',''))):raise ValueError('Campagna archiviata non valida')
                tick=row.get('last_tick')
                if type(tick) is not int or tick<0:raise ValueError('Data campagna non valida')
        for guide,key in active.items():
            if key not in runs or runs[key]['guide_id']!=guide:raise ValueError('Partita attiva non valida')
        for guide,bindings in campaigns.items():
            if not isinstance(bindings,dict):raise ValueError('Indice campagne non valido')
            for identity,key in bindings.items():
                row=runs.get(key,{})
                if row.get('guide_id')!=guide or identity!=self._campaign_key(row.get('campaign_id'),row.get('country')):raise ValueError('Associazione campagna non valida')
        self._runs,self._active,self._campaign_active=deepcopy(runs),dict(active),deepcopy(campaigns)
        for row in self._runs.values():
            if not row.get('campaign_id'):row['campaign_required']=True
        self._restored_ids=set(runs)

    @staticmethod
    def _campaign_key(campaign,country):
        return json.dumps([campaign,country],separators=(',',':'))

    def _new(self, guide_id, session_id, checks, reserved):
        _identifier(guide_id, 'Identità guida')
        _identifier(session_id, 'Sessione', optional=True)
        run_id = self._id_factory()
        _identifier(run_id, 'Identità partita', maximum=160)
        if run_id in reserved:
            raise ValueError('Identità partita duplicata')
        return {'schema_version': 1, 'guide_id': guide_id, 'run_id': run_id,
                'session_id': session_id, 'checks': dict(checks), 'started': {}, 'run_revision':0}

    @_mutation
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

    @_mutation
    def create(self, guide_id, session_id=None):
        """Activate an empty run and retain every previous run for this guide."""
        with self._lock:
            record = self._new(guide_id, session_id, {}, self._runs)
            old=self._runs.get(self._active.get(guide_id),{})
            for key in ('campaign_id','country','last_tick'):
                if key in old:record[key]=old[key]
            self._runs[record['run_id']] = record
            self._active[guide_id] = record['run_id']
            if record.get('campaign_id'):
                self._campaign_active.setdefault(guide_id,{})[self._campaign_key(record['campaign_id'],record['country'])]=record['run_id']
            return deepcopy(record)

    @_mutation
    def select(self, guide_id, run_id):
        """Activate a historical run only when it belongs to the same guide."""
        with self._lock:
            record = self.get(guide_id, run_id)
            current=self.get(guide_id)
            if current.get('campaign_id') and (record.get('campaign_id'),record.get('country'))!=(current.get('campaign_id'),current.get('country')):
                raise ValueError('Questa checklist appartiene a un’altra campagna o nazione')
            if record.get('campaign_id') and self._tick is not None and record.get('last_tick',0)>self._tick:
                raise ValueError('Questa checklist contiene avanzamento successivo alla partita caricata')
            self._active[guide_id] = record['run_id']
            if record.get('campaign_id'):
                record['session_id']=current['session_id'];self._runs[run_id]=record
                self._campaign_active.setdefault(guide_id,{})[self._campaign_key(record['campaign_id'],record['country'])]=run_id
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

    @_mutation
    def update(self, guide_id, checks, *, started=None):
        """Replace the active run's checklist after validating the whole update."""
        checks = _checks(checks)
        started = _checks(started) if started is not None else None
        with self._lock:
            record = self.get(guide_id)
            record['checks'] = checks
            if started is not None:
                record['started'] = started
            self._runs[record['run_id']] = record
            return deepcopy(record)

    @_mutation
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
                if current.get('campaign_id'):continue
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

    def bind_campaign(self,campaign_id,country,session_id,tick):
        if not valid_id(campaign_id) or not isinstance(country,str) or not re.fullmatch('[A-Z0-9]{3}',country):raise ValueError('Identità campagna non qualificata')
        _identifier(session_id,'Sessione')
        if type(tick) is not int or tick<0:raise ValueError('Data campagna non qualificata')
        with self._lock:
            binding=(campaign_id,country,session_id)
            # Time moving forward alone is not a disk write or a new run.
            if self._binding==binding and self._tick is not None and tick>=self._tick and all(
                    row.get('campaign_id')==campaign_id and row.get('country')==country
                    for row in (self._runs[key] for key in self._active.values())):
                self._tick=tick;return False
            old_binding,old_tick=self._binding,self._tick
            try:
                changed=self._bind_campaign(campaign_id,country,session_id,tick)
                self._binding=binding;self._tick=tick
                return changed
            except Exception:
                self._binding,self._tick=old_binding,old_tick
                raise

    @_mutation
    def _bind_campaign(self,campaign_id,country,session_id,tick):
        index=self._campaign_key(campaign_id,country);changed=False
        for guide,active in list(self._active.items()):
            key=self._campaign_active.get(guide,{}).get(index)
            current=self._runs[active]
            record=deepcopy(self._runs[key]) if key else None
            if record is None:
                if not current.get('campaign_id') and current['run_id'] not in self._restored_ids and current['session_id'] in (None,session_id):record=deepcopy(current)
                else:record=self._new(guide,session_id,{},self._runs)
            last=self._tick if self._binding and self._binding[:2]==(campaign_id,country) else record.get('last_tick')
            if last is not None and tick<last:
                record=self._new(guide,session_id,{},self._runs);record['branch_reason']='timeline_rewind'
            record.update(campaign_id=campaign_id,country=country,session_id=session_id,last_tick=tick)
            if record!=current:changed=True
            self._runs[record['run_id']]=record;self._active[guide]=record['run_id']
            self._campaign_active.setdefault(guide,{})[index]=record['run_id']
        return changed

    def history(self,guide_id):
        with self._lock:
            return [deepcopy({k:v for k,v in row.items() if k not in ('checks','started','baselines')})
                    for row in self._runs.values() if row['guide_id']==guide_id]

    @_mutation
    def observe(self, guide, live_state):
        """Capture actionable construction/production counts once per run.

        Queued construction or pending production assignments cannot establish
        an earlier count. Established baselines are never revised.
        All reads come from supplied evidence, with no game or clock access.
        """
        with self._lock:
            record = self.get(guide['id'])
            candidates = {step['key']: relative_spec(step) or production_spec(step) for rows in guide['tracks'].values()
                          for step in rows if step['key'] not in record.get('baselines', {})}
            candidates = {key: spec for key, spec in candidates.items() if spec is not None}
            if not candidates:
                return False
            state = normalise_state(live_state)
            result = evaluate(guide, state, record)
            if (result['mode'] != 'live' or result['progress_status'] != 'accepted'
                    or not record['session_id'] or record['session_id'] != state['session_id']
                    or not isinstance(result['country'], str) or not result['country']):
                return False
            queued = state['facts'].get('construction.queued', {})
            queue_known = queued.get('status') in ('observed', 'calculated') and bool(queued.get('source')) and isinstance(queued.get('value'), list)
            changed = False
            statuses = {row['key']: row['status'] for row in result['steps']}
            occupied = {entry['fact_key'] for key, entry in record.get('baselines', {}).items()
                        if statuses.get(key) not in ('completed', 'not_applicable')}
            for row in result['steps']:
                spec = candidates.get(row['key'])
                if (spec is None or 'queue_key' in spec and (not queue_known or spec['queue_key'] in queued['value'])
                        or spec['fact_key'] in occupied
                        or row['reason'] not in ('prerequisites_satisfied', 'manual_confirmation_required')):
                    continue
                if 'queue_key' in spec:
                    current = observed_count(state, spec['fact_key'])
                else:
                    metrics = row.get('production', {})
                    count = metrics.get('assigned')
                    current = {'value':count,'source':'production_adapter'} if type(count) is int and metrics.get('requested') == count else None
                if current is None:
                    continue
                record.setdefault('baselines', {})[row['key']] = {
                    'fact_key': spec['fact_key'], 'value': current['value'], 'quantity': spec['quantity'],
                    'country': result['country'], 'source': current['source']}
                occupied.add(spec['fact_key'])
                changed = True
            if changed:
                self._runs[record['run_id']] = record
            return changed

    def metadata(self):
        """Return active identities without duplicating checklist data in SSE."""
        with self._lock:
            return {guide_id: {key: value for key, value in self._runs[run_id].items() if key not in ('checks', 'started', 'baselines')}
                    for guide_id, run_id in self._active.items()}
