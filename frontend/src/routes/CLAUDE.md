# src/routes/

TanStack Router file routes. A route maps a URL to a feature view and nothing more: thin by
design. The router plugin turns this folder into `src/app/routeTree.gen.ts` on `pnpm dev` and
`pnpm build`; commit the regenerated file with your route change.

| File                                  | URL                                                                                                                             |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `__root.tsx`                          | Root layout: `AppShell` with the primary navigation and the top bar title                                                       |
| `index.tsx`                           | `/` → redirects to `/agents`                                                                                                    |
| `agents/index.tsx`                    | `/agents`: `AgentsOverview`                                                                                                     |
| `agents/new.tsx`                      | `/agents/new`: `CreateAgentPage` (title "Create agent")                                                                         |
| `agents/$agentId/provision.tsx`       | `/agents/:agentId/provision`: `ProvisionPage` (title "Provision")                                                               |
| `settings.tsx`                        | `/settings`                                                                                                                     |
| `agents/$agentId/plan.tsx`            | `/agents/:agentId/plan`: `AgentHeader` and `PlanPage` (title "Plan"); a save refreshes progress                                 |
| `agents/$agentId/test.tsx`            | `/agents/:agentId/test`: `AgentHeader` and `TestPage` (title "Test"); a saved scenario set or a finished run refreshes progress |
| (later) `agents/$agentId/publish.tsx` | The Publish step; it starts with `AgentHeader`, whose stepper links to it                                                       |

The provision route also composes the testing feature's `TestCallPanel` into `ProvisionPage`
(its `testCall` slot, beside the settings): the quick test call stays there next to the
settings, and the Test step has its own route for scenarios.

## May import

- `src/features/<name>` through its `index.ts` only (`feature-public-api-only`).
- `src/shared/ui`, `src/shared/lib`, `@tanstack/react-router`, `lucide-react`.
- Not `src/app/`, not `src/shared/api/`, not a feature's `views/`, `state/` or `api/`.

## Conventions

- One `createFileRoute` per file. Name the path exactly as the file (the plugin checks it).
- Set `staticData: { title }` on every page route: it fills the top bar `<h1>` and the tab title.
- Validate search params with `validateSearch` and a Zod schema with defaults, so a bad URL
  renders defaults instead of an error (Postel).
- Load data with `loader: ({ context }) => context.queryClient.ensureQueryData(...)` using
  query options exported by the feature.
- Primary navigation lives in `__root.tsx`: **Agents first, Settings last, 7 items at most**
  (serial position, Miller). The app shell contract test pins this.
- Files that are not routes start with `-` (the plugin ignores them), but prefer putting
  helpers in a feature or `shared/`.

## Testing

- Routes have no logic of their own; `src/app/app.test.tsx` covers them through the app.
- Every new page gets `auditScreen(page)` coverage in an E2E spec (`e2e/`).
- Coverage counts toward the 80 % frontend floor.

## Example

```tsx
// src/routes/agents/$agentId/plan.tsx (M3)
import { createFileRoute } from '@tanstack/react-router';

import { PlanEditor, planQueryOptions } from '../../../features/plans';

export const Route = createFileRoute('/agents/$agentId/plan')({
  staticData: { title: 'Plan' },
  loader: ({ context, params }) =>
    context.queryClient.ensureQueryData(planQueryOptions(params.agentId)),
  component: PlanEditor,
});
```
