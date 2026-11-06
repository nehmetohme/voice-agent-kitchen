/**
 * What views may know about the API: the view-model types, the contract's limits and the pure
 * readers of API errors. Views never import `api/` (dependency-cruiser `views-not-to-api`), so
 * they come through here.
 */
export { PLAN_LIMITS, isNotFound } from '../api/plans-api';
export type {
  LockedText,
  Path,
  PlanTarget,
  PlanView,
  Problem,
  Step,
  StepKind,
} from '../api/plans-api';
