"""Version 1 domain contracts. Adapters never acquire or mutate game state.

JSON-compatible TypedDicts deliberately avoid dependencies on parsers, HTTP,
native layouts or rendering. See docs/framework-contracts.md for invariants.
"""
from __future__ import annotations

from copy import deepcopy
import math
import re
from typing import Any, Literal, NotRequired, TypeAlias, TypedDict

VERSION = 1
EXPERIENCE_CURRENCIES = {'army_experience':'country.experience.army','navy_experience':'country.experience.navy',
                         'air_experience':'country.experience.air'}
SPIRIT_CURRENCIES = {group:currency for currency,groups in (
    ('army_experience',('academy_spirit','army_spirit','division_command_spirit')),
    ('navy_experience',('naval_academy_spirit','navy_spirit','naval_command_spirit')),
    ('air_experience',('air_force_spirit','air_force_command_spirit','air_force_academy_spirit')))
    for group in groups}
MILITARY_SPIRIT_GROUPS = frozenset(SPIRIT_CURRENCIES)
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
    depends_on_started: NotRequired[list[str]]
    political_power_cost: NotRequired[int | float]
    resource_costs: NotRequired[dict[str, int | float]]
    requirements: NotRequired[Condition]
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
    superseded_by: NotRequired[list[str]]
    basis: NotRequired[Literal['requested','assigned','remaining']]
    assigned: int | None
    requested: int | None
    active: int | None
    damaged: int | None
    baseline: int | None
    target: int | None
    lines: list[ProductionLine]
    reason: str


class ProductionLayout(TypedDict):
    layout: list[ProductionProgress]


class OperationBaseline(TypedDict):
    fact_key: str
    value: int
    country: str
    quantity: int
    source: str


ConstructionBaseline = OperationBaseline


class CampaignObservations(TypedDict):
    country: str
    war_seen: NotRequired[Literal[True]]
    milestones: NotRequired[dict[str, Literal[True]]]


class GuideProgress(TypedDict):
    schema_version: Literal[1]
    guide_id: str
    run_id: str
    session_id: NotRequired[str | None]
    campaign_id: NotRequired[str]
    campaign_required: NotRequired[bool]
    country: NotRequired[str]
    last_tick: NotRequired[int]
    run_revision: NotRequired[int]
    checks: NotRequired[dict[str, bool]]
    started: NotRequired[dict[str, bool]]
    baselines: NotRequired[dict[str, OperationBaseline]]
    campaign_conditions: NotRequired[CampaignObservations]
    research_bindings: NotRequired[dict[str, str]]
    research_history: NotRequired[dict[str, str]]
    research_options: NotRequired[dict[str, list[str]]]


StepStatus = Literal['completed', 'active', 'ready', 'waiting', 'not_applicable', 'unverifiable']


class PoliticalPowerRequirement(TypedDict):
    cost: int | float | None
    available: int | float | None
    affordable: bool | None
    cost_basis: Literal['guide_declared', 'unknown']


class ActionReadiness(TypedDict):
    domain: Literal['political', 'project', 'appointment']
    value: bool | None
    unknown: list[str]
    blocked: list[str]
    political_power: NotRequired[PoliticalPowerRequirement]
    resources_available: bool | None
    resources: list[dict]
    requirements_declared: bool


class ActionSequence(TypedDict):
    political: str | None
    projects: list[str]


class ActionContext(TypedDict):
    session_id: str | None
    campaign_id: str | None


class FortificationProgress(TypedDict):
    target_level: int | None
    building_id: Literal['bunker', 'coastal_bunker']
    province_ids: list[int]
    completed_provinces: int | None
    total_provinces: int | None


class RuntimeStep(TypedDict):
    fortifications: NotRequired[FortificationProgress]
    research_target: NotRequired[str]
    focus_readiness: NotRequired[FocusReadiness]
    production_admitted: NotRequired[bool]
    superseded_by: NotRequired[list[str]]
    original_completion: NotRequired[Fact]
    original_selection: NotRequired[Fact]
    key: str
    track: str
    status: StepStatus
    tracking: NotRequired[Literal['reminder']]
    reason: str
    blocked_by: list[str]
    completion: Fact
    construction: NotRequired[dict[str, int | None]]
    production: NotRequired[ProductionProgress | ProductionLayout]
    selection: Fact
    availability: Fact
    action_readiness: NotRequired[ActionReadiness]
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
    optional_steps: NotRequired[list[str]]
    alternative_steps: NotRequired[list[str]]


class GuideEvaluation(TypedDict):
    focus_plan: NotRequired[FocusPlan]
    schema_version: Literal[1]
    guide_id: str
    run_id: str | None
    progress_status: Literal['none', 'accepted', 'wrong_guide', 'wrong_session', 'session_unverified', 'wrong_campaign', 'campaign_unverified']
    mode: Literal['live', 'preview', 'wrong_country', 'wrong_tree']
    country: str | None
    steps: list[RuntimeStep]
    tracks: dict[str, RuntimeTrack]
    ready: list[str]
    parallel: dict[Literal['focus', 'research'], RuntimeLane]
    action_sequence: ActionSequence
    action_context: ActionContext


class GuideProjection(TypedDict):
    """Coherent API snapshot; state retains the existing transport envelope."""
    schema_version: Literal[1]
    engine: Literal['rust-guide-core']
    server_id: str
    guide_id: str
    definition_revision: str
    state: dict[str, Any]
    progress: GuideProgress
    evaluation: GuideEvaluation


class FocusReadiness(TypedDict):
    value: bool | None
    available: bool | None
    branch: bool | None
    prerequisites: bool | None
    excluded: bool | None
    checks: list[dict[str, Any]]
    reason: str


class FocusPlan(TypedDict):
    primary: str
    blocked: FocusReadiness | dict[str, Any]
    steps: list[str]


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


def normalise_state(value: dict) -> LiveState:
    """Pure Rust v1 normalization; stale and missing evidence remains unknown."""
    from .backend_core import _request
    return _request(0,'state_normalize',value=value)


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
