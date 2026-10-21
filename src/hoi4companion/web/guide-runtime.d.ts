import type {GuideEvaluation, GuideProgress} from './contracts';
/** Pure evaluation. Does not execute actions, acquire state, or mutate inputs. */
export function evaluate(guide: unknown, state: unknown, progress?: GuideProgress|null): GuideEvaluation;
