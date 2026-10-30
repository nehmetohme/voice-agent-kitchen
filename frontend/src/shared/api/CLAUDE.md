# src/shared/api/

The typed client for the backend REST API, generated from the contract so the two sides
cannot drift.

| Path         | What                                                                                                                                                          |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `generated/` | **orval output** from `contracts/openapi.json`: fetch functions, TanStack Query hooks, Zod schemas, MSW handlers. Never edit; regenerate with `just openapi`. |
| `http.ts`    | The fetch mutator every generated call goes through                                                                                                           |

`generated/` appears with task T0.4, when the backend first emits `contracts/openapi.json`.
`verify` never runs orval; CI checks that the committed client matches the contract.

## What `http.ts` guarantees

- Same-origin credentials (Databricks Apps SSO cookie) and a JSON `Accept` header.
- Non-2xx → `ApiError` with RFC 9457 `problem` details (`status`, `title`, `detail`).
- 2xx → the body is parsed with the Zod schema orval passes (`includeZodSchemaInArguments`).
  A mismatch throws `ContractViolationError` (Postel: strict about what we accept from code,
  loud when the backend breaks the contract).

## May import / be imported by

- Imports: `zod` and the platform `fetch` only. Never React, never `features/`.
- Imported by: `features/*/api/` only (`generated-client-only-from-feature-api`). Views and
  state never call the API directly (`views-not-to-api`).

## Conventions

- Add cross-cutting request behaviour (auth header, tracing header, retries) in `http.ts`,
  never in features.
- Do not hand-write DTO types; use the generated ones, then map to view models in
  `features/*/api/`.
- Errors shown to users come from `problem.detail`; never show raw response bodies.

## Testing

- `http.test.ts` stubs `fetch` and covers success, contract violation and error mapping.
- Feature `api/` tests use the generated MSW handlers.
- Coverage counts toward the 80 % frontend floor; `generated/` is excluded.

## Example

```ts
// features/agents/api/agents-api.ts (once generated/ exists)
import { listAgents as listAgentsDto } from '../../../shared/api/generated/endpoints/agents';

/** View model: what the feature's state and views work with (api/ never imports them). */
export interface Agent {
  id: string;
  name: string;
  live: boolean;
}

export async function listAgents(): Promise<Agent[]> {
  const page = await listAgentsDto();
  return page.items.map((dto) => ({ id: dto.id, name: dto.name, live: dto.liveVersion !== null }));
}
```
