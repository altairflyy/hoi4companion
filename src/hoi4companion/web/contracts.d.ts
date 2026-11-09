/** Version 1 JSON domain contracts. Runtime implementation: contracts.js. */
export type EntityKind = 'country'|'focus'|'technology'|'state'|'equipment'|'project'|'building';
export type EntityRef = {kind: EntityKind; id: string; scope?: string};
export type Entity = {schema_version: 1; ref: EntityRef; name: string; properties: Record<string, unknown>};
export type Fact<T = unknown> =
  | {status: 'observed'|'calculated'; value: T; source: string; reason?: string}
  | {status: 'unknown'; value: null; source: ''; reason?: string};
export type LiveState = {schema_version: 1; usable: boolean; session_id: string|null; facts: Record<string, Fact>};
export type Condition =
  | {op: 'all'|'any'; args: Condition[]}
  | {op: 'not'; arg: Condition}
  | {op: 'fact'; key: string; test: 'eq'|'contains'|'gte'; value: string|number|boolean}
  | {op: 'check'; key: string};
export type JsonValue = null|string|number|boolean|JsonValue[]|{[key: string]: JsonValue};
export type GuideSource = {id: string; type: 'youtube'|'web'|'file'|'manual'; url?: string};
export type GuidePhase = {id: string; title: string; when?: Condition};
export type GuideEvidence = {source_id: string; basis: 'explicit'|'visual'|'inferred'|'game_derived'|'unverified'; timestamp_seconds?: number; end_seconds?: number};
export type Action =
  | {kind: 'select_focus'; entity: EntityRef & {kind: 'focus'}; params?: Record<string, JsonValue>}
  | {kind: 'research'; entity: EntityRef & {kind: 'technology'}; params?: Record<string, JsonValue>}
  | {kind: 'special_project'; entity: EntityRef & {kind: 'project'}; params?: Record<string, JsonValue>}
  | {kind: 'manual'|'free'; params?: Record<string, JsonValue>};
export type GuideStep = {
  key: string; action: Action; title?: string; note?: string; confirmation?: string;
  tracking?: 'action'|'reminder';
  when?: Condition; priority_when?: Condition; priority?: number; skip_when?: Condition;
  phase?: string; depends_on?: string[]; depends_on_started?: string[]; political_power_cost?: number; resource_costs?: Record<string, number>; requirements?: Condition; completion?: Condition; started_when?: Condition;
  evidence?: GuideEvidence[];
  focus_criteria?: {schema_version: 1; available: FocusCriterion; branch: FocusCriterion;
    prerequisites: string[][]; mutex: string[]; conflicts: string[]};
  focus_resolution?: {schema_version: 1; position: number; query: string;
    classification: 'direct'|'reconstructed'|'probable'|'unresolved'; entity: EntityRef & {kind: 'focus'}; locked?: boolean};
};
export type FocusCriterion = {op:'const';value:boolean}|{op:'unknown';trigger:string;value?:string}
  |{op:'all'|'any';args:FocusCriterion[]}|{op:'not';arg:FocusCriterion}
  |{op:'if';condition:FocusCriterion;then:FocusCriterion}
  |{op:'compare';fact:string;test:'>'|'<'|'>='|'<='|'='|'!='|'contains';value:string|number|boolean;trigger:string};
/** Evaluated rows retain the legacy completion fact; the definition stays separate. */
export type EvaluatedGuideStep = Omit<GuideStep, 'completion'> & {
  completion: Fact<boolean>; completion_condition?: Condition;
  selection: Fact<boolean>; availability: Fact<boolean>;
};
export type Guide = {
  schema_version: 1; id: string; title: string; subtitle?: string;
  runtime_engine?: 'rust-guide-core'; runtime_definition_revision?: string;
  applies_to: {country?: string; tree_id?: string};
  tracks: Record<string, GuideStep[]>;
  sources?: GuideSource[];
  phases?: GuidePhase[];
  focus_sequence?: {schema_version: 1; coverage: 'partial'|'complete'; initial_focuses?: string[]};
  milestones?: {key: string; title: string; note?: string; confirmation?: string; seconds?: number}[];
};
export type EquipmentDesign = {status: 'observed'; modules: Record<string, string>; upgrades: Record<string, number>}|{status: 'unavailable'; modules?: never; upgrades?: never};
export type DesignDifference = {kind: 'modules'|'upgrades'; key: string; expected: string|number|null; actual: string|number|null};
export type DesignCheck = {status: 'unknown'|'match'|'different'|'missing'; candidates: {line_id: string; name: string; status: 'unknown'|'match'|'different'; differences: DesignDifference[]}[]};
export type ProductionLine = {id: string; equipment: string; equipment_name?: string; name: string; category: 'military'|'naval'; requested_factories: number; assigned_factories: number; active_factories: number; damaged_factories: number; output_per_day?: number; design?: EquipmentDesign};
export type PoliticalIdea = {id: string; group: string; category: string; is_law: boolean; name?: string};
export type PoliticalAppointment = {id: string; character_id: string; slot: number; name?: string};
export type VisibleEvent = {instance_id: string; event_id: string; option_ids?: string[]; name?: string};
export type OperativeMissionId = 'no_mission'|'build_intel_network'|'quiet_network'|'counter_intelligence'|'root_out_resistance'|'boost_ideology'|'control_trade'|'diplomatic_pressure'|'propaganda';
export type OperativeAssignment = {id:string; mission_id:OperativeMissionId; target_country:string|null; state:'captured'|'cooldown'|'disbanding'|'active'|'operation'|'killed'};
export type ForeignInteractions = {status:'observed';countries:string[];improving_relations:string[];non_aggression_pacts:string[]}|{status:'unavailable';reason:string};
export type OperativeAssignments = {status:'observed';countries:string[];items:OperativeAssignment[]}|{status:'unavailable';reason:string};
export type ProductionRevision = {group: string; order: number};
export type ProductionProgress = {superseded_by?: string[]; assigned: number|null; requested: number|null; active: number|null; damaged: number|null; baseline: number|null; target: number|null; lines: ProductionLine[]; reason: string; basis?: 'requested'|'assigned'|'remaining'; design_check?: DesignCheck};
export type ProductionLayout = {layout: ProductionProgress[]};
export type OperationBaseline = {fact_key: string; value: number; country: string; quantity: number; source: string};
export type ConstructionBaseline = OperationBaseline;
export type CampaignObservations = {country: string; war_seen?: true; milestones?: Record<string, true>};
export type ProductionMilestone = {step: string; token: string; condition: Condition};
export type GuideProgress = {
  research_bindings?: Record<string, string>;
  research_history?: Record<string, string>;
  research_options?: Record<string, string[]>;
  campaign_conditions?: CampaignObservations;
  schema_version: 1; guide_id: string; run_id: string;
  session_id?: string|null; checks?: Record<string, boolean>; started?: Record<string, boolean>;
  campaign_id?: string; campaign_required?: boolean; country?: string; last_tick?: number; run_revision?: number;
  baselines?: Record<string, OperationBaseline>;
};
export type StepStatus = 'completed'|'active'|'ready'|'waiting'|'not_applicable'|'unverifiable';
export type RuntimeStep = {
  research_target?: string;
  production_admitted?: boolean;
  superseded_by?: string[]; original_completion?: Fact<boolean>; original_selection?: Fact<boolean>;
  key: string; track: string; status: StepStatus; reason: string; blocked_by: string[];
  tracking?: 'reminder';
  completion: Fact<boolean>; selection: Fact<boolean>; availability: Fact<boolean>;
  construction?: {baseline: number|null; current: number|null; target: number|null};
  fortifications?: {target_level: number|null; building_id: 'bunker'|'coastal_bunker'; province_ids: number[]; completed_provinces: number|null; total_provinces: number|null};
  production?: ProductionProgress | ProductionLayout;
  action_readiness?: {domain: 'political'|'project'|'appointment'; value: boolean|null; unknown: string[]; blocked: string[]; resources_available: boolean|null; requirements_declared: boolean; resources: {key: string; cost: number; available: number|null; affordable: boolean|null}[]; political_power?: {cost: number|null; available: number|null; affordable: boolean|null; cost_basis: 'guide_declared'|'unknown'}};
  focus_readiness?: FocusReadiness;
  completion_basis: 'native_membership'|'manual'|'condition'|'free'|'reminder'; priority: number; priority_known: boolean;
};
export type RuntimeTrack = {keys: string[]; order: string[]; next: string|null};
export type FocusReadiness = {value:boolean|null;available:boolean|null;branch:boolean|null;prerequisites:boolean|null;excluded:boolean|null;checks:Record<string,JsonValue>[];reason:string};
export type RuntimeLane = {capacity: number|null; steps: string[]; optional_steps?:string[];alternative_steps?:string[]};
export type GuideEvaluation = {
  focus_plan?: {primary:string;blocked:FocusReadiness|Record<string,never>;steps:string[]};
  schema_version: 1; guide_id: string; run_id: string|null;
  progress_status: 'none'|'accepted'|'wrong_guide'|'wrong_session'|'session_unverified'|'wrong_campaign'|'campaign_unverified';
  mode: 'live'|'preview'|'wrong_country'|'wrong_tree'; country: string|null;
  steps: RuntimeStep[]; tracks: Record<string, RuntimeTrack>; ready: string[];
  parallel: {focus: RuntimeLane; research: RuntimeLane};
  action_context: {session_id: string|null; campaign_id: string|null};
  action_sequence: {political: string|null; projects: string[]};
};
/** Coherent API snapshot. Normalize the legacy transport with state() for display. */
export type GuideProjection = {
  schema_version: 1; engine: 'rust-guide-core'; server_id: string;
  guide_id: string; definition_revision: string;
  state: LiveState|{usable: boolean; sample?: Record<string, unknown>|null; [key: string]: unknown};
  progress: GuideProgress & {revision: number; server_id: string};
  evaluation: GuideEvaluation;
};
export function state(value: unknown): LiveState;
export function fact(state: LiveState, key: string): Fact;
export function condition(node: Condition, state: LiveState, checks?: Record<string, boolean>): boolean|null;
export function guide(value: unknown): Guide;
export function checkKeys(node?: Condition): string[];
export const VERSION: 1;
