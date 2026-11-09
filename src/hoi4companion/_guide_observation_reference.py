"""Rust-owned campaign progression; transitional qualified-observation adapters.

The transient observation buffer is not authoritative storage. Native mutations
validate identity, update revisions, and retain rollback state until persistence
has succeeded. No operation acquires game data or changes a guide definition.
"""
from copy import deepcopy
from functools import wraps
from .progress_core import NativeRuns
from ._construction_progress_reference import relative_spec, observed_count
from ._contracts_reference import normalise_state
from .guide_runtime import evaluate
from ._production_progress_reference import relative_spec as production_spec
from .campaign_conditions import uses_after_war, war_value


def _mutation(method):
    @wraps(method)
    def transition(self,*args,**kwargs):
        with self._lock:
            before=self._call('snapshot')['runs']
            self._observation_runs=deepcopy(before)
            try:
                value=method(self,*args,**kwargs)
                records={key:row for key,row in self._observation_runs.items() if before.get(key)!=row}
                if records:return self._call('observe_replace',records=records,value=value)
                return value
            finally:self._observation_runs=None
    return transition


class RunStore(NativeRuns):
    @property
    def _runs(self):
        # Only the existing evidence adapter reads its own transient buffer.
        # Generic run/campaign operations are implemented entirely in Rust.
        return self._observation_runs if getattr(self,'_observation_runs',None) is not None else self._call('snapshot')['runs']

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

