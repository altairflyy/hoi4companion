"""Version 1 domain contracts. Adapters never acquire or mutate game state.

JSON-compatible TypedDicts deliberately avoid dependencies on parsers, HTTP,
native layouts or rendering. See docs/framework-contracts.md for invariants.
"""
from __future__ import annotations

from copy import deepcopy
import math
from typing import Any, Literal, NotRequired, TypeAlias, TypedDict

VERSION = 1
EntityKind = Literal['country', 'focus', 'technology', 'state', 'equipment', 'project', 'building']
Knowledge = Literal['observed', 'calculated', 'unknown']


class EntityRef(TypedDict):
    kind: EntityKind
    id: str
    scope: NotRequired[str]


class Entity(TypedDict):
    schema_version: Literal[1]
    ref: EntityRef
    name: str
    properties: dict[str, Any]


class KnownFact(TypedDict):
    status: Literal['observed', 'calculated']
    value: Any
    source: str
    reason: NotRequired[str]


class UnknownFact(TypedDict):
    status: Literal['unknown']
    value: None
    source: Literal['']
    reason: NotRequired[str]


Fact: TypeAlias = KnownFact | UnknownFact


class GroupCondition(TypedDict):
    op: Literal['all', 'any']
    args: list[Condition]


class NotCondition(TypedDict):
    op: Literal['not']
    arg: Condition


class FactCondition(TypedDict):
    op: Literal['fact']
    key: str
    test: Literal['eq', 'contains', 'gte']
    value: str | int | float | bool


class CheckCondition(TypedDict):
    op: Literal['check']
    key: str


Condition: TypeAlias = GroupCondition | NotCondition | FactCondition | CheckCondition
JsonValue: TypeAlias = str | int | float | bool | None | list['JsonValue'] | dict[str, 'JsonValue']


class GuideSource(TypedDict):
    id: str
    type: Literal['youtube', 'web', 'file', 'manual']
    url: NotRequired[str]


class GuidePhase(TypedDict):
    id: str
    title: str
    when: NotRequired[Condition]


class GuideEvidence(TypedDict):
    source_id: str
    basis: Literal['explicit', 'visual', 'inferred', 'game_derived', 'unverified']
    timestamp_seconds: NotRequired[int | float]
    end_seconds: NotRequired[int | float]


class Action(TypedDict):
    kind: Literal['select_focus', 'research', 'special_project', 'manual', 'free']
    entity: NotRequired[EntityRef]
    params: NotRequired[dict[str, JsonValue]]


class LiveState(TypedDict):
    schema_version: Literal[1]
    usable: bool
    session_id: str | None
    facts: dict[str, Fact]


class GuideStep(TypedDict):
    key: str
    action: Action
    tracking: NotRequired[Literal['action', 'reminder']]
    when: NotRequired[Condition]
    priority_when: NotRequired[Condition]
    priority: NotRequired[int]
    skip_when: NotRequired[Condition]
    title: NotRequired[str]
    note: NotRequired[str]
    confirmation: NotRequired[str]
    phase: NotRequired[str]
    depends_on: NotRequired[list[str]]
    completion: NotRequired[Condition]
    started_when: NotRequired[Condition]
    evidence: NotRequired[list[GuideEvidence]]


class GuideScope(TypedDict, total=False):
    country: str
    tree_id: str


class Guide(TypedDict):
    schema_version: Literal[1]
    id: str
    applies_to: GuideScope
    tracks: dict[str, list[GuideStep]]
    title: str
    subtitle: NotRequired[str]
    sources: NotRequired[list[GuideSource]]
    phases: NotRequired[list[GuidePhase]]


class ProductionLine(TypedDict):
    id: str
    equipment: str
    equipment_name: NotRequired[str]
    name: str
    category: Literal['military', 'naval']
    requested_factories: int
    assigned_factories: int
    active_factories: int
    damaged_factories: int
    output_per_day: NotRequired[float]


class PoliticalIdea(TypedDict):
    id: str
    group: str
    category: str
    is_law: bool
    name: NotRequired[str]


class PoliticalAppointment(TypedDict):
    id: str
    character_id: str
    slot: int  # Native code; catalog supplies role names, not an enum guess.
    name: NotRequired[str]


class VisibleEvent(TypedDict):
    instance_id: str
    event_id: str
    option_ids: NotRequired[list[str]]
    name: NotRequired[str]


class ProductionProgress(TypedDict):
    assigned: int | None
    requested: int | None
    active: int | None
    damaged: int | None
    baseline: int | None
    target: int | None
    lines: list[ProductionLine]
    reason: str


class OperationBaseline(TypedDict):
    fact_key: str
    value: int
    country: str
    quantity: int
    source: str


ConstructionBaseline = OperationBaseline


class GuideProgress(TypedDict):
    schema_version: Literal[1]
    guide_id: str
    run_id: str
    session_id: NotRequired[str | None]
    checks: NotRequired[dict[str, bool]]
    started: NotRequired[dict[str, bool]]
    baselines: NotRequired[dict[str, OperationBaseline]]


StepStatus = Literal['completed', 'active', 'ready', 'waiting', 'not_applicable', 'unverifiable']


class RuntimeStep(TypedDict):
    key: str
    track: str
    status: StepStatus
    tracking: NotRequired[Literal['reminder']]
    reason: str
    blocked_by: list[str]
    completion: Fact
    construction: NotRequired[dict[str, int | None]]
    production: NotRequired[ProductionProgress]
    selection: Fact
    availability: Fact
    completion_basis: Literal['native_membership', 'manual', 'condition', 'free', 'reminder']
    priority: int
    priority_known: bool


class RuntimeTrack(TypedDict):
    keys: list[str]
    order: list[str]
    next: str | None


class RuntimeLane(TypedDict):
    capacity: int | None
    steps: list[str]


class GuideEvaluation(TypedDict):
    schema_version: Literal[1]
    guide_id: str
    run_id: str | None
    progress_status: Literal['none', 'accepted', 'wrong_guide', 'wrong_session', 'session_unverified']
    mode: Literal['live', 'preview', 'wrong_country', 'wrong_tree']
    country: str | None
    steps: list[RuntimeStep]
    tracks: dict[str, RuntimeTrack]
    ready: list[str]
    parallel: dict[Literal['focus', 'research'], RuntimeLane]


def is_json_number(value) -> bool:
    """Finite interoperable JSON numbers, with exact integers in both runtimes."""
    if type(value) is int:
        return abs(value) <= 2**53 - 1
    return type(value) is float and math.isfinite(value) and (not value.is_integer() or abs(value) <= 2**53 - 1)


def fact(value: Any = None, status: Knowledge = 'unknown', source: str = '', *, reason: str = '') -> Fact:
    if status not in ('observed', 'calculated', 'unknown'):
        raise ValueError('Stato conoscenza non valido')
    if status == 'unknown' and value is not None:
        raise ValueError('Un dato sconosciuto non può avere un valore')
    if status == 'unknown' and source != '':
        raise ValueError('Un dato sconosciuto non può avere una sorgente osservata')
    if status != 'unknown' and (value is None or not isinstance(source, str) or not source):
        raise ValueError('Valore noto senza provenienza')
    if isinstance(value, float) and not math.isfinite(value):
        raise ValueError('Numero non finito')
    return {'status': status, 'value': value, 'source': source, **({'reason': reason} if reason else {})}


def validate_entity(entity: EntityRef) -> None:
    if not isinstance(entity, dict) or entity.get('kind') not in EntityKind.__args__:
        raise ValueError('Tipo entità non valido')
    if not isinstance(entity.get('id'), str) or not 0 < len(entity['id']) <= 512:
        raise ValueError('Identità entità non valida')
    if 'scope' in entity and (not isinstance(entity['scope'], str) or not entity['scope']):
        raise ValueError('Ambito entità non valido')


def entity(ref: EntityRef, name: str, properties: dict | None = None) -> Entity:
    validate_entity(ref)
    if not isinstance(name, str) or not isinstance(properties or {}, dict):
        raise ValueError('Entità non valida')
    return {'schema_version': VERSION, 'ref': deepcopy(ref), 'name': name, 'properties': deepcopy(properties or {})}


def validate_action(action: Action) -> None:
    if not isinstance(action, dict) or action.get('kind') not in ('select_focus', 'research', 'special_project', 'manual', 'free'):
        raise ValueError('Azione non supportata')
    expected = {'select_focus': 'focus', 'research': 'technology', 'special_project': 'project'}.get(action['kind'])
    if expected:
        validate_entity(action.get('entity'))
        if action['entity']['kind'] != expected:
            raise ValueError('Entità incompatibile con azione')
    elif 'entity' in action:
        raise ValueError('Azione descrittiva senza entità richiesta')


def validate_condition(value: Condition, depth: int = 0) -> None:
    if depth > 16 or not isinstance(value, dict):
        raise ValueError('Condizione non valida o troppo profonda')
    op = value.get('op')
    if op in ('all', 'any'):
        args = value.get('args')
        if not isinstance(args, list) or not 1 <= len(args) <= 64:
            raise ValueError('Operandi condizione non validi')
        for child in args:
            validate_condition(child, depth + 1)
    elif op == 'not':
        validate_condition(value.get('arg'), depth + 1)
    elif op in ('fact', 'check'):
        if not isinstance(value.get('key'), str) or not 0 < len(value['key']) <= 512:
            raise ValueError('Chiave condizione non valida')
        if op == 'fact' and (value.get('test') not in ('eq', 'contains', 'gte') or 'value' not in value):
            raise ValueError('Confronto condizione non valido')
        if op == 'fact' and (type(value['value']) not in (str, int, float, bool) or
                             type(value['value']) in (int, float) and not is_json_number(value['value'])):
            raise ValueError('Valore condizione non scalare o non finito')
    else:
        raise ValueError('Operatore condizione non supportato')


def _valid_fact_type(key, value):
    if key in ('politics.ideas','politics.advisors','events.visible','events.visible_options','decisions.listed','decisions.executed'):
        return (isinstance(value,list) and len(value)<=4096 and all(isinstance(x,str) and 0<len(x)<=512 for x in value)
                and len(set(value))==len(value))
    if key == 'production.lines':
        return _valid_production_lines(value)
    if key in ('country.intelligence.is_created', 'country.intelligence.in_creation'):
        return type(value) is bool
    if key in ('country', 'focus.tree'):
        return isinstance(value, str) and bool(value)
    if key in ('focus.completed', 'focus.active', 'focus.continuous', 'focus.ids', 'research.completed', 'research.active', 'research.ids', 'construction.queued'):
        return isinstance(value, list) and all(isinstance(item, str) and item for item in value)
    if key == 'research.slots':
        return isinstance(value, list) and all(isinstance(item, dict) and 'technology_id' in item and
                (item['technology_id'] is None or isinstance(item['technology_id'], str) and bool(item['technology_id'])) for item in value)
    return True


def _valid_production_lines(lines):
    if not isinstance(lines, list) or len(lines) > 4096:
        return False
    identities = set()
    for row in lines:
        if (not isinstance(row, dict) or any(not isinstance(row.get(key), str) or not row[key]
                for key in ('id', 'equipment', 'name')) or row['id'] in identities
                or row.get('category') not in ('military', 'naval')
                or any(type(row.get(key)) not in (int, float) or not is_json_number(row[key])
                       or row[key] != int(row[key]) or not 0 <= row[key] <= 9007199254740991
                       for key in ('requested_factories', 'assigned_factories', 'active_factories', 'damaged_factories'))
                or row['assigned_factories'] != row['active_factories'] + row['damaged_factories']
                or row['assigned_factories'] > row['requested_factories']):
            return False
        identities.add(row['id'])
    return True


def _queued_construction(section):
    """Project a complete qualified queue into membership, never completions."""
    if not isinstance(section, dict) or section.get('status') != 'observed' or not isinstance(section.get('lines'), list):
        return None
    keys, identities = set(), set()
    def integer(value, minimum=0):
        return type(value) in (int, float) and is_json_number(value) and value == int(value) and value >= minimum
    for row in section['lines']:
        if (not isinstance(row, dict) or any(not isinstance(row.get(key), str) or not row[key] for key in ('id', 'building_id'))
                or row['id'] in identities or row.get('location_status') != 'observed'
                or not integer(row.get('state_id'), 1)
                or ('province_id' not in row or row['province_id'] is not None and not integer(row['province_id'], 1))
                or any(not integer(row.get(key)) for key in ('amount', 'active_factories', 'built_level', 'repair_levels'))
                or type(row.get('conversion')) is not bool):
            return None
        identities.add(row['id'])
        if row['amount'] > 0 and row['repair_levels'] == 0 and not row['conversion']:
            keys.add('building:' + row['building_id'])
            keys.add(f"state:{int(row['state_id'])}:{row['building_id']}")
    return sorted(keys)


def normalise_state(value: dict) -> LiveState:
    """Convert the legacy transport at the consumer seam; stale bytes stay unknown.

    Empty membership is known only when supplied and not explicitly incomplete.
    Absence of an active selection is known via an empty slots list / null focus,
    represented by active-ID lists (known empty), never by an unknown null fact.
    """
    value = value if isinstance(value, dict) else {}
    if 'schema_version' in value:
        if not is_json_number(value['schema_version']) or value['schema_version'] != VERSION or value.get('usable') is not True or not isinstance(value.get('facts'), dict):
            return {'schema_version': VERSION, 'usable': False, 'session_id': None, 'facts': {}}
        facts = {}
        for key, item in value.get('facts', {}).items():
            try:
                if not _valid_fact_type(key, item.get('value')) and item.get('status') != 'unknown':
                    raise ValueError('Tipo fatto non valido')
                facts[key] = fact(item.get('value'), item.get('status'), item.get('source'), reason=item.get('reason', ''))
            except (ValueError, AttributeError):
                facts[key] = fact(reason='Dato contratto non valido')
        return {'schema_version': VERSION, 'usable': True, 'session_id': value.get('session_id'), 'facts': facts}
    sample = value.get('sample') if value.get('usable') is True else None
    sample = sample if isinstance(sample, dict) else {}
    usable = bool(sample)
    facts = {}
    source = value.get('source') or 'process_memory'
    def put(key, data, known=True, calculated=False, reason='Campo non osservato'):
        facts[key] = fact(data, 'calculated' if calculated else 'observed', source if not calculated else 'legacy_adapter') if known and data is not None and _valid_fact_type(key, data) else fact(reason=reason)
    put('country', sample.get('country'))
    put('focus.tree', sample.get('focus_tree_id'), calculated=True)
    put('focus.completed', sample.get('completed_focuses'), sample.get('focus_complete') is not False)
    put('research.completed', sample.get('researched_technologies'), sample.get('research_complete') is not False)
    put('focus.active', [sample['current_focus']] if sample.get('current_focus') else [], 'current_focus' in sample)
    put('focus.continuous', [sample['continuous_focus']] if sample.get('continuous_focus') else [], 'continuous_focus' in sample)
    slots = sample.get('research_slots')
    slots = slots if _valid_fact_type('research.slots', slots) else None
    put('research.active', [s['technology_id'] for s in slots if s.get('technology_id')] if isinstance(slots, list) else None)
    put('research.slots', slots)
    for key, native in [('focus.ids', 'focus_ids'), ('research.ids', 'technology_ids')]:
        put(key, sample.get(native))
    overview = sample.get('country_overview') or {}
    overview = overview if isinstance(overview, dict) else {}
    queued = _queued_construction(overview.get('construction')) if isinstance(sample.get('country'), str) and sample['country'] and overview.get('country') == sample['country'] else None
    facts['construction.queued'] = fact(queued, 'calculated', 'construction_adapter') if queued is not None else fact(reason='Coda costruzioni non qualificata')
    production = overview.get('production', {})
    lines = production.get('lines') if isinstance(production, dict) else None
    production_known = (isinstance(sample.get('country'), str) and bool(sample['country'])
                        and overview.get('country') == sample['country'] and isinstance(production, dict)
                        and production.get('status') == 'observed' and _valid_production_lines(lines))
    facts['production.lines'] = fact(deepcopy(lines), 'calculated', 'production_adapter') if production_known else fact(reason='Linee di produzione non qualificate')
    facts.update(_political_facts(overview if isinstance(sample.get('country'),str) and sample['country'] and overview.get('country')==sample['country'] else {}))
    if overview.get('country') == sample.get('country'):
        # These fields are derivations in country.py, not direct observations.
        derived = {'by_type', 'by_template', 'total', 'effective_factories', 'output_per_day',
                   'assigned_factories', 'category', 'balance', 'used', 'owned', 'controlled', 'name'}
        def flatten(path, data, known, calculated=False):
            if isinstance(data, dict):
                for key, child in data.items():
                    if key not in ('status', 'reason'):
                        flatten(path + '.' + key, child, known, calculated or key in derived)
            elif isinstance(data, list):
                put(path + '.count', len(data), known, True)
                for index, child in enumerate(data):
                    identifier = child.get('id', str(index)) if isinstance(child, dict) else str(index)
                    flatten(path + '.' + str(identifier), child, known, calculated)
            else:
                aggregated_stockpile = path.startswith('country.equipment.items.') and path.endswith('.stockpile')
                enumerated_line = path.startswith('country.production.lines.') and path.endswith('.id')
                construction_label = path.startswith('country.construction.lines.') and path.rsplit('.', 1)[-1] in ('position', 'location_status', 'state_name')
                put(path, data, known, calculated or aggregated_stockpile or enumerated_line or construction_label)
        for section, data in overview.items():
            if isinstance(data, dict):
                # Available manpower is a computed sum; explicit zero is known.
                if section == 'manpower':
                    for key, child in data.items():
                        if key not in ('status', 'reason'):
                            put('country.manpower.' + key, child, data.get('status') in ('observed', 'partial'), key == 'available')
                else:
                    flatten('country.' + section, data, data.get('status') in ('observed', 'partial'))
    return {'schema_version': VERSION, 'usable': usable, 'session_id': sample.get('session_id'), 'facts': facts}


def _political_facts(overview):
    """Only complete identity lists can prove membership or known absence."""
    result={key:fact(reason='Sezione politica non qualificata') for key in
            ('politics.ideas','politics.advisors','events.visible','events.visible_options','decisions.listed','decisions.executed')}
    def identifier(value):
        import re
        return isinstance(value,str) and 0<len(value)<=160 and re.fullmatch(r'[A-Za-z0-9_.-]+',value) is not None
    def publish(key,values): result[key]=fact(sorted(set(values)),'calculated','politics_adapter')
    politics=overview.get('politics',{})
    if isinstance(politics,dict) and politics.get('status') in ('observed','partial'):
        for part in ('ideas','advisors'):
            section=politics.get(part,{})
            rows=section.get('items') if isinstance(section,dict) and section.get('status')=='observed' else None
            if not isinstance(rows,list) or len(rows)>4096: continue
            ids=[]
            for row in rows:
                if not isinstance(row,dict) or not identifier(row.get('id')): break
                if part=='ideas' and (not identifier(row.get('group')) or not identifier(row.get('category')) or type(row.get('is_law')) is not bool): break
                if part=='advisors' and (not identifier(row.get('character_id')) or type(row.get('slot')) is not int or not 0<=row['slot']<=2_000_000_000): break
                ids.append(row['id'])
            else:
                if len(set(ids))==len(ids): publish('politics.'+part,ids)
    events=overview.get('events',{})
    rows=events.get('visible') if isinstance(events,dict) and events.get('status')=='observed' else None
    if isinstance(rows,list) and len(rows)<=256:
        identities=set(); visible=[]; options=[]; options_known=True
        for row in rows:
            if (not isinstance(row,dict) or not identifier(row.get('instance_id')) or row['instance_id'] in identities
                    or not identifier(row.get('event_id'))): break
            identities.add(row['instance_id']); visible.append(row['event_id'])
            choices=row.get('option_ids')
            if not isinstance(choices,list) or len(choices)>256 or not all(identifier(x) for x in choices) or len(set(choices))!=len(choices): options_known=False
            else: options.extend(row['event_id']+':'+choice for choice in choices)
        else:
            publish('events.visible',visible)
            if options_known: publish('events.visible_options',options)
    decisions=overview.get('decisions',{})
    listed=decisions.get('listed_ids') if isinstance(decisions,dict) and decisions.get('status')=='observed' else None
    if isinstance(listed,list) and len(listed)<=4096 and all(identifier(x) for x in listed) and len(set(listed))==len(listed):
        publish('decisions.listed',listed)
    return result


def evaluate_condition(node: Condition, state: LiveState, checks: dict[str, bool] | None = None) -> bool | None:
    """Small declarative core, not a Clausewitz interpreter or action executor."""
    validate_condition(node)
    state = normalise_state(state)
    checks = checks or {}
    def evaluate(item):
        op = item['op']
        if op in ('all', 'any'):
            values = [evaluate(child) for child in item['args']]
            if op == 'all':
                return False if False in values else None if None in values else True
            return True if True in values else None if None in values else False
        if op == 'not':
            value = evaluate(item['arg'])
            return None if value is None else not value
        if op == 'check':
            value = checks.get(item['key'])
            return value if type(value) is bool else None
        observed = state['facts'].get(item['key'], fact()) if state['usable'] else fact()
        if observed['status'] == 'unknown':
            return None
        left, right = observed['value'], item['value']
        if item['test'] == 'contains':
            return any((type(x) is type(right) or type(x) in (int, float) and type(right) in (int, float)) and x == right for x in left) if isinstance(left, list) else None
        if item['test'] == 'eq':
            if isinstance(left, (list, dict)) or isinstance(right, (list, dict)):
                return None
            same_type = type(left) is type(right) or type(left) in (int, float) and type(right) in (int, float)
            return left == right if same_type else None
        if item['test'] == 'gte':
            return left >= right if type(left) in (int, float) and type(right) in (int, float) else None
        return None
    return evaluate(node)
