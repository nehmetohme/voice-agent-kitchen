// What the testing views may know about the API: view models and the contract's limits. Views
// may not import `api/`, even for types (features/CLAUDE.md).
export type { RubricCheck, RubricItem, Scenario, ScenarioResult } from '../api/test-runs-api';
export { SCENARIO_LIMITS, isNotFound, startProblemOf } from '../api/test-runs-api';
