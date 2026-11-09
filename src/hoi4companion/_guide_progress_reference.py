"""Legacy campaign-state test oracle, never a runtime fallback."""
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
from .campaign_conditions import uses_after_war, validate_observations, war_value


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
                            if method.__name__ in ('update','_observe_candidates','_observe_campaign_conditions','create','select') and self._binding and (row.get('campaign_id'),row.get('country'),row.get('session_id'))==self._binding:
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
            validate_observations(row.get('campaign_conditions',{}))
            for field in ('research_bindings','research_history'):
                bindings = row.get(field, {})
                if not isinstance(bindings,dict) or len(bindings)>4096 or any(not isinstance(k,str) or not k or not isinstance(v,str) or not v or len(v)>512 for k,v in bindings.items()):
                    raise ValueError('Associazioni ricerca archiviate non valide')
            offers=row.get('research_options',{})
            if (not isinstance(offers,dict) or len(offers)>4096 or any(not isinstance(k,str) or not k or
                    not isinstance(v,list) or not 1<=len(v)<=8 or any(not isinstance(x,str) or not 0<len(x)<=512 for x in v)
                    or len(v)!=len(set(v)) for k,v in offers.items())):raise ValueError('Alternative ricerca archiviate non valide')
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
                history=dict(record.get('research_history',{}))
                for step,target in record.get('research_bindings',{}).items():history.setdefault(step,target)
                record=self._new(guide,session_id,{},self._runs);record['branch_reason']='timeline_rewind'
                if history:record['research_history']=history
            record.update(campaign_id=campaign_id,country=country,session_id=session_id,last_tick=tick)
            if record!=current:changed=True
            self._runs[record['run_id']]=record;self._active[guide]=record['run_id']
            self._campaign_active.setdefault(guide,{})[index]=record['run_id']
        return changed

    def history(self,guide_id):
        with self._lock:
            return [deepcopy({k:v for k,v in row.items() if k not in ('checks','started','baselines')})
                    for row in self._runs.values() if row['guide_id']==guide_id]

    def observe(self, guide, live_state, *, allow_research=True):
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
            has_upgrades = allow_research and any(step.get('relative_research') or step.get('research_choice') for rows in guide['tracks'].values() for step in rows)
            from .production_milestones import definitions as milestone_definitions
            history=record.get('campaign_conditions',{})
            needs_war=uses_after_war(guide) and history.get('war_seen') is not True
            needs_milestones=any(token not in history.get('milestones',{}) for token in milestone_definitions(guide))
            changed = self._observe_campaign_conditions(guide,live_state,record) if needs_war or needs_milestones else False
            if has_upgrades:
                changed = self._observe_upgrades(guide,live_state,self.get(guide['id'])) or changed
            if candidates:
                changed = self._observe_candidates(guide, live_state, self.get(guide['id']), candidates) or changed
            return changed

    @_mutation
    def _observe_campaign_conditions(self,guide,live_state,record):
        state=normalise_state(live_state)
        result=evaluate(guide,state,record)
        current=war_value(state)
        if (result['mode']!='live'
                or result['progress_status']!='accepted' or not record.get('session_id')
                or record['session_id']!=state.get('session_id')):
            return False
        observed=deepcopy(record.get('campaign_conditions',{}))
        if current is not None and current[1] is True:observed.update(country=current[0],war_seen=True)
        from .production_milestones import definitions as milestone_definitions
        completed={r['key'] for r in result['steps'] if r['status']=='completed'}
        from .contracts import evaluate_condition
        for token,milestone in milestone_definitions(guide).items():
            if milestone.get('foreign_interaction'):
                # A later action's desired state may already be true. Record
                # it only when its declared context and dependencies admit it.
                row=next(r for rows in guide['tracks'].values() for r in rows if r['key']==milestone['step'])
                results={r['key']:r for r in result['steps']}
                conditions=[row[k] for k in ('when','requirements') if k in row]
                conditions.extend(p['when'] for p in guide.get('phases',[]) if p['id']==row.get('phase') and 'when' in p)
                if (any(evaluate_condition(c,state,{}) is not True for c in conditions)
                        or 'skip_when' in row and evaluate_condition(row['skip_when'],state,{}) is not False
                        or any(results[k]['status']!='completed' for k in row.get('depends_on',[]))
                        or any(results[k]['selection']['value'] is not True and results[k]['completion']['value'] is not True for k in row.get('depends_on_started',[]))):
                    continue
            if milestone['step'] in completed and evaluate_condition(milestone['condition'],state,{}) is True:
                observed['country']=result['country'];observed.setdefault('milestones',{})[token]=True
        if not observed:return False
        if record.get('campaign_conditions')==observed:return False
        record['campaign_conditions']=observed
        self._runs[record['run_id']]=record
        return True

    @_mutation
    def _observe_upgrades(self, guide, live_state, record):
        if not any(step.get('relative_research') or step.get('research_choice') for rows in guide['tracks'].values() for step in rows):
            return False
        state = normalise_state(live_state)
        base = evaluate(guide, state, record)
        if (base['mode'] != 'live' or base['progress_status'] != 'accepted'
                or not record.get('session_id') or record['session_id'] != state.get('session_id')):
            return False
        facts = state['facts']
        values = []
        for key in ('research.completed', 'research.active'):
            fact = facts.get(key, {})
            if fact.get('status') != 'observed' or not fact.get('source') or not isinstance(fact.get('value'), list):
                return False
            values.append(set(fact['value']))
        completed, active = values
        bindings = record.setdefault('research_bindings', {})
        changed = False
        # A rewind creates new progress, not a new meaning for the same UP.
        # Recover only target identities; completion always comes from this save.
        history = dict(record.get('research_history', {}))
        if record.get('branch_reason') == 'timeline_rewind' and 'research_history' not in record and record.get('campaign_id'):
            for prior in self._runs.values():
                if prior['run_id'] == record['run_id']:break
                if (prior['guide_id'],prior.get('campaign_id'),prior.get('country')) != (record['guide_id'],record['campaign_id'],record.get('country')):continue
                for field in ('research_history','research_bindings'):
                    for key,target in prior.get(field,{}).items():history.setdefault(key,target)
            if history:record['research_history']=history;changed=True
        for rows in guide['tracks'].values():
            for step in rows:
                chain=step.get('relative_research',{}).get('chain',[])
                saved=history.get(step['key'])
                if saved in chain and step['key'] in bindings and bindings[step['key']] != saved:
                    del bindings[step['key']];changed=True
        for rows in guide['tracks'].values():
            previous = []
            for step in rows:
                if step.get('research_choice'):
                    if step['key'] not in bindings:
                        from .research_choices import context as choice_context
                        info=choice_context(guide,state,bindings,record.get('research_options',{}))[step['key']]
                        if step['research_choice'].get('mode')=='once' and info['options'] and info['options']!=record.get('research_options',{}).get(step['key']):
                            pending=next(r for r in evaluate(guide,state,record)['steps'] if r['key']==step['key'])
                            if pending['status']=='ready':
                                record.setdefault('research_options',{})[step['key']]=list(info['options']);changed=True
                                info=choice_context(guide,state,bindings,record['research_options'])[step['key']]
                        if info['branch'] and len(info['options'])==1:
                            pending=next(r for r in evaluate(guide,state,record)['steps'] if r['key']==step['key'])
                            if pending['status']!='ready':previous.append(step);continue
                            trial=deepcopy(record);trial.setdefault('research_bindings',{})[step['key']]=info['options'][0]
                            assessed=evaluate(guide,state,trial)
                            row=next(r for r in assessed['steps'] if r['key']==step['key'])
                            if row['status'] in ('ready','active','completed'):
                                bindings[step['key']]=info['options'][0];changed=True
                    previous.append(step);continue
                rel = step.get('relative_research')
                if not rel or step['key'] in bindings:
                    if step['action']['kind'] == 'research':
                        previous.append(step)
                    continue
                chain = rel['chain']
                used = set()
                for item in previous:
                    if item.get('relative_research', {}).get('chain') == chain and item['key'] not in bindings:
                        break
                    ident = bindings.get(item['key']) if item.get('relative_research') else item['action']['entity']['id']
                    if ident in chain:
                        if ident not in completed:
                            break
                        used.add(ident)
                else:
                    saved=history.get(step['key'])
                    target = saved if saved in chain else next((x for x in chain if x not in completed and x not in used), None)
                    if target:
                        trial = deepcopy(record)
                        trial.setdefault('research_bindings', {})[step['key']] = target
                        assessed = evaluate(guide, state, trial)
                        row = next(r for r in assessed['steps'] if r['key'] == step['key'])
                        if row['status'] in ('ready', 'active') or saved in chain and row['status']=='completed':
                            bindings[step['key']] = target
                            changed = True
                previous.append(step)
        if changed:
            self._runs[record['run_id']] = record
        return changed

    @_mutation
    def _observe_candidates(self, guide, live_state, record, candidates):
        # The caller holds the same reentrant lock from candidate selection
        # through commit; existing rollback and persistence remain transactional.
        with self._lock:
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
                    requested=spec['fact_key'].startswith('production_requested:')
                    count = metrics.get('requested' if requested else 'assigned')
                    current = {'value':count,'source':'production_adapter'} if type(count) is int and (requested or metrics.get('requested') == count) else None
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
