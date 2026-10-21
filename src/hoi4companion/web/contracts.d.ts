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
  when?: Condition; priority_when?: Condition; priority?: number; skip_when?: Condition;
  phase?: string; depends_on?: string[]; completion?: Condition;
  evidence?: GuideEvidence[];
};
/** Evaluated rows retain the legacy completion fact; the definition stays separate. */
export type EvaluatedGuideStep = Omit<GuideStep, 'completion'> & {
  completion: Fact<boolean>; completion_condition?: Condition;
  selection: Fact<boolean>; availability: Fact<boolean>;
};
export type Guide = {
  schema_version: 1; id: string; title: string; subtitle?: string;
  applies_to: {country?: string; tree_id?: string};
  tracks: Record<string, GuideStep[]>;
  sources?: GuideSource[];
  phases?: GuidePhase[];
  milestones?: {key: string; title: string; note?: string; confirmation?: string; seconds?: number}[];
};
export function state(value: unknown): LiveState;
export function fact(state: LiveState, key: string): Fact;
export function condition(node: Condition, state: LiveState, checks?: Record<string, boolean>): boolean|null;
export function guide(value: unknown): Guide;
export function checkKeys(node?: Condition): string[];
export const VERSION: 1;
