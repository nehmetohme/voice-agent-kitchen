/**
 * What views may know about the API: the view-model types and the pure readers of API errors.
 * Views never import `api/` (dependency-cruiser `views-not-to-api`), so they come through here.
 */
export { fieldErrorsOf, isNotFound } from '../api/agents-api';
export type { AgentDetails, AgentListItem, AgentSettings, FieldErrors } from '../api/agents-api';
