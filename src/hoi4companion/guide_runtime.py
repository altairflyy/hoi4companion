"""Pure Guide Schema v1 interpreter, independent of acquisition and presentation.

The runtime describes the next declared actions. It neither invents strategy nor
executes actions, and recomputes completion from the supplied evidence each call.
"""
from __future__ import annotations

from collections import deque

from .contracts import GuideEvaluation, GuideProgress, evaluate_condition, is_json_number, normalise_state
from .guide import manual_keys, normalise_guide
from .construction_progress import validate_baselines, construction_context
from .production_progress import production_context


def _unknown(reason='not_observed'):
    return {'status': 'unknown', 'value': None, 'source': '', 'reason': reason}


def _calculated(value, source):
    return {'status': 'calculated', 'value': value, 'source': source}


def _value(state, key):
    item = state['facts'].get(key)
    if (state['usable'] and isinstance(item, dict) and item.get('status') in ('observed', 'calculated')
            and item.get('source') and item.get('value') is not None):
        return item['value']
    return None


def _membership(state, key, target):
    values = _value(state, key)
    if not isinstance(values, list) or any(not isinstance(item, str) for item in values):
        return _unknown()
    return _calculated(target in values, 'membership:' + key)


def _progress(guide, state, value):
    if value is None:
        return 'none', None, {}, {}
    if (not isinstance(value, dict) or not is_json_number(value.get('schema_version'))
            or value['schema_version'] != 1 or not isinstance(value.get('guide_id'), str)
            or not isinstance(value.get('run_id'), str) or not value['run_id'].strip()
            or len(value['run_id']) > 160 or not isinstance(value.get('checks', {}), dict)):
        raise ValueError('Avanzamento della partita non valido')
    checks = value.get('checks', {})
    if any(not isinstance(key, str) or type(checked) is not bool for key, checked in checks.items()):
        raise ValueError('Conferme della partita non valide')
    started = value.get('started', {})
    if not isinstance(started, dict) or any(not isinstance(key, str) or type(flag) is not bool for key, flag in started.items()):
        raise ValueError('Avvii della partita non validi')
    binding = value.get('session_id')
    if binding is not None and (not isinstance(binding, str) or not binding.strip()):
        raise ValueError('Sessione della partita non valida')
    if value['guide_id'] != guide.get('id', ''):
        return 'wrong_guide', None, {}, {}
    campaign=value.get('campaign_id')
    if value.get('campaign_required') and campaign is None:
        return 'campaign_unverified',None,{},{}
    if campaign is not None:
        from .campaign import valid_id
        if not valid_id(campaign):raise ValueError('Campagna della partita non valida')
        observed=state['facts'].get('campaign.id',{})
        if not state['usable'] or observed.get('status')!='observed' or not observed.get('source'):
            return 'campaign_unverified',None,{},{}
        if observed.get('value')!=campaign or value.get('country')!=_value(state,'country'):
            return 'wrong_campaign',None,{},{}
    if binding is not None:
        if not state['usable']:
            return 'session_unverified', None, {}, {}
        if binding != state.get('session_id'):
            return 'wrong_session', None, {}, {}
    allowed = manual_keys(guide)
    operations = {step['key'] for rows in guide['tracks'].values() for step in rows
                  if step['action']['kind'] in ('manual', 'special_project') and step.get('tracking') != 'reminder'}
    return ('accepted', value['run_id'], {key: checked for key, checked in checks.items() if key in allowed},
            {key: flag for key, flag in started.items() if key in operations})


def _topological(rows):
    pending = {step['key']: len(step.get('depends_on', [])) for _, step in rows}
    successors = {key: [] for key in pending}
    for _, step in rows:
        for dependency in step.get('depends_on', []):
            successors[dependency].append(step['key'])
    queue = deque(key for key, count in pending.items() if not count)
    result = []
    while queue:
        key = queue.popleft()
        result.append(key)
        for other in successors[key]:
            pending[other] -= 1
            if not pending[other]:
                queue.append(other)
    # normalise_guide validates the graph; keep this guard for a clear contract.
    if len(result) != len(rows):
        raise ValueError('Dipendenze della guida cicliche')
    return result


def evaluate(guide, state, progress: GuideProgress | None = None) -> GuideEvaluation:
    """Return deterministic statuses and bounded parallel proposals for a guide.

    ``progress`` is explicitly bound to a guide and a caller-owned run identity;
    its optional session binding prevents carrying confirmations across games.
    Unknown evidence remains unknown, including under condition negation.
    """
    guide = normalise_guide(guide)
    state = normalise_state(state)
    country = _value(state, 'country')
    tree = _value(state, 'focus.tree')
    scope = guide['applies_to']
    mode = 'live' if state['usable'] else 'preview'
    if state['usable']:
        if scope.get('country') and country is not None and country != scope['country']:
            mode = 'wrong_country'
        elif scope.get('tree_id') and tree is not None and tree != scope['tree_id']:
            mode = 'wrong_tree'
        elif (scope.get('country') and country is None) or (scope.get('tree_id') and tree is None):
            mode = 'preview'
    progress_status, run_id, checks, started = _progress(guide, state, progress)
    baselines = validate_baselines(progress.get('baselines', {})) if progress is not None else {}
    mismatched = mode in ('wrong_country', 'wrong_tree')
    if mismatched:
        checks = {}
        started = {}
    context = state if mode == 'live' else {'schema_version': 1, 'usable': False, 'session_id': None, 'facts': {}}
    context, construction = construction_context(guide, context, baselines if progress_status == 'accepted' else {})
    context, production = production_context(guide, context, baselines if progress_status == 'accepted' else {})
    # JSON objects have no semantic order. Canonical track IDs also avoid JS's
    # special property order for integer-like names; list order stays authored.
    track_items = sorted(guide['tracks'].items())
    rows = [(track, step) for track, steps in track_items for step in steps]
    definitions = {step['key']: step for _, step in rows}
    phases = {phase['id']: phase for phase in guide.get('phases', [])}
    results = {}
    implicit = {}
    for track, steps in track_items:
        barriers = []
        for step in steps:
            implicit[step['key']] = list(barriers) if 'depends_on' not in step else []
            if step['action']['kind'] == 'manual' and step.get('tracking') != 'reminder':
                barriers.append(step['key'])

    def condition(node):
        return evaluate_condition(node, context, checks)

    for track, step in rows:
        key = step['key']
        kind = step['action']['kind']
        ref = step['action'].get('entity', {})
        native = kind in ('select_focus', 'research')
        reminder = step.get('tracking') == 'reminder'
        basis = 'reminder' if reminder else 'condition' if 'completion' in step else 'native_membership' if native else 'free' if kind == 'free' else 'manual'
        completion, selection = _unknown('not_tracked' if reminder else 'not_observed'), _unknown()
        entity_mismatch = (kind == 'select_focus' and ref.get('scope') and tree is not None and ref['scope'] != tree)
        entity_unknown = kind == 'select_focus' and ref.get('scope') and tree is None
        if not reminder and not mismatched and not entity_mismatch and not entity_unknown:
            if native:
                prefix = 'focus' if kind == 'select_focus' else 'research'
                selection = _membership(context, prefix + '.active', ref['id'])
                completion = _membership(context, prefix + '.completed', ref['id'])
            elif kind in ('manual', 'special_project') and key in checks:
                completion = _calculated(checks[key], 'check:' + key)
            if 'started_when' in step:
                value = condition(step['started_when'])
                selection = _unknown() if value is None else _calculated(value, 'declared_start')
            elif kind in ('manual', 'special_project') and key in started:
                selection = _calculated(started[key], 'manual_started:' + key)
            if 'completion' in step:
                value = condition(step['completion'])
                completion = _unknown() if value is None else _calculated(value, 'declared_completion')
        priority_known = True
        priority = step.get('priority', 0)
        if 'priority_when' in step:
            value = condition(step['priority_when'])
            priority_known = value is not None
            if value is not True:
                priority = 0
        results[key] = {'key': key, 'track': track, 'status': 'unverifiable', 'reason': '',
                        'blocked_by': [], 'completion': completion, 'selection': selection,
                        'completion_basis': basis, 'priority': priority, 'priority_known': priority_known,
                        'availability': _unknown('dynamic_availability_not_evaluated')}
        result = results[key]
        if reminder:
            result['tracking'] = 'reminder'
        if key in construction:
            result['construction'] = construction[key]
        if key in production:
            result['production'] = production[key]
        if mismatched:
            result.update(status='not_applicable', reason='guide_scope_mismatch')
        elif entity_mismatch:
            result.update(status='not_applicable', reason='entity_scope_mismatch')
        elif entity_unknown:
            result.update(reason='entity_scope_unknown')
        elif completion['value'] is True:
            result.update(status='completed', reason='completion_true')
        elif selection['value'] is True:
            result.update(status='active', reason='selection_true')
        elif mode != 'live' and native:
            result.update(reason='live_state_unavailable')
        else:
            result['reason'] = 'pending'

    for key in _topological(rows):
        result = results[key]
        if result['reason'] != 'pending':
            continue
        step = definitions[key]
        kind = step['action']['kind']
        if 'skip_when' in step:
            value = condition(step['skip_when'])
            if value is not False:
                result.update(status='not_applicable' if value is True else 'unverifiable',
                              reason='skip_condition_true' if value is True else 'skip_condition_unknown')
                continue
        conditions = []
        phase = phases.get(step.get('phase'))
        if phase and 'when' in phase:
            conditions.append((phase['when'], 'phase_condition'))
        if 'when' in step:
            conditions.append((step['when'], 'condition'))
        failed = False
        for node, reason in conditions:
            value = condition(node)
            if value is not True:
                result.update(status='waiting' if value is False else 'unverifiable',
                              reason=reason + ('_false' if value is False else '_unknown'))
                failed = True
                break
        if failed:
            continue
        if step.get('catalog_match') is False:
            result.update(reason='catalog_unmatched')
            continue
        blocked = [dependency for dependency in step.get('depends_on', []) if results[dependency]['status'] != 'completed']
        unknown = any(results[dependency]['status'] == 'unverifiable' for dependency in blocked)
        for dependency in implicit[key]:
            completion = results[dependency]['completion']['value']
            if completion is not True:
                blocked.append(dependency)
                unknown = unknown or completion is None
        if blocked:
            result.update(status='unverifiable' if unknown else 'waiting',
                          reason='dependency_unknown' if unknown else 'dependency_not_completed', blocked_by=blocked)
        elif kind in ('select_focus', 'research') and result['selection']['value'] is None:
            result.update(reason='selection_unknown')
        elif step.get('tracking') == 'reminder':
            result.update(status='ready', reason='reminder_available')
        elif result['completion']['value'] is None and kind != 'free':
            result.update(reason='manual_confirmation_required' if kind in ('manual', 'special_project') else 'completion_unknown')
        else:
            result.update(status='ready', reason='free_choice' if kind == 'free' else 'prerequisites_satisfied')

    positions = {step['key']: index for index, (_, step) in enumerate(rows)}
    def ordering(key):
        return -results[key]['priority'], positions[key]
    tracks = {}
    for track, steps in track_items:
        keys = [step['key'] for step in steps]
        order = sorted(keys, key=ordering)
        next_key = next((key for key in order if definitions[key].get('tracking') != 'reminder'
                         and results[key]['status'] not in ('completed', 'not_applicable', 'active')), None)
        tracks[track] = {'keys': keys, 'order': order, 'next': next_key}
    focus_active = _value(context, 'focus.active')
    slots = _value(context, 'research.slots')
    focus_capacity = (0 if focus_active else 1) if isinstance(focus_active, list) else None
    research_capacity = sum(not slot.get('technology_id') for slot in slots) if isinstance(slots, list) else None
    parallel = {'focus': {'capacity': focus_capacity, 'steps': []},
                'research': {'capacity': research_capacity, 'steps': []}}
    candidates = {'focus': [], 'research': []}
    for track in tracks.values():
        for key in track['order']:
            result = results[key]
            kind = definitions[key]['action']['kind']
            if definitions[key].get('tracking') == 'reminder' or kind == 'special_project' or result['status'] in ('completed', 'active', 'not_applicable'):
                continue
            if result['status'] in ('waiting', 'unverifiable'):
                if result['reason'] in ('dependency_not_completed', 'dependency_unknown'):
                    continue
                break
            if kind in ('select_focus', 'research'):
                candidates['focus' if kind == 'select_focus' else 'research'].append(key)
    for kind in parallel:
        capacity = parallel[kind]['capacity']
        parallel[kind]['steps'] = sorted(candidates[kind], key=ordering)[:capacity] if capacity is not None else []
    return {'schema_version': 1, 'guide_id': guide.get('id', ''), 'run_id': run_id,
            'progress_status': progress_status, 'mode': mode, 'country': country,
            'steps': [results[step['key']] for _, step in rows], 'tracks': tracks,
            'ready': sorted((key for key, result in results.items() if result['status'] == 'ready'
                             and definitions[key].get('tracking') != 'reminder'), key=ordering),
            'parallel': parallel}
