# src/features/

One folder per step of the product flow. Each feature is a small vertical slice with its own
layers. This file covers every feature and its `views/`, `state/` and `api/` folders.

| Feature       | Step                                                                                          | Milestone |
| ------------- | --------------------------------------------------------------------------------------------- | --------- |
| `agents/`     | Provision: list, create (3 fields), edit with autosave, archive, the agent header and stepper | M2        |
| `plans/`      | Plan: flow editor (steps, paths, located problems, autosave); later guardrails, knowledge     | M3        |
| `testing/`    | Test: live call, transcript, latency, call summary (M4, M5); scenarios and test runs (M5)     | M4, M5    |
| `publishing/` | Publish: versions, channels, rollback                                                         | M6        |

**Agent workspace** (contract `agent-workspace` v1): each step of an agent starts with
`AgentHeader` (name, `SetupStepper` as the steps' navigation, "More actions" with archiving).
`ProvisionPage` fits the viewport from `lg`: settings left, and the route's `TestCallPanel`
right (`testCall` slot; the quick test call stays there in M5). Its transcript orders lines by
turn: `state/test-call.ts` reserves the agent's line on `agent.audio.start` and fills it on
`transcript.final`. An ended call shows `CallSummary` under the header (PE-1): turns, this
call's reply p50/p95 (`state/call-summary.ts`, nearest rank), "Run tests" and "Publish"
("Later" until M6).

**Testing** (`testing/`, M5): `TestPage` (`views/test-page.tsx`) is the Test step, a workspace
that fits the screen from `lg`: `RunBar` (the run's outcome as a polite status, its progress
while it runs, "Run tests" or "Cancel run"; the stale notice when the run tested an earlier
draft), then `ScenarioList` (each row selects a scenario and shows "Required" and its latest
result; "Add scenario") beside `ScenarioResult` (the selected scenario's name with its Edit and
Delete in `ScenarioActions`, how it ended, `RubricChecks`, then its transcript in
`CallTranscript` with "Caller" lines, opened at the start: `follow={false}`). `state/use-test-page.ts`
reads the scenarios, the agent's latest run (read again every second while it runs) and calls
`onChange` when the set is saved or a run ends; `state/run-view.ts` and
`state/scenario-edits.ts` hold the rules in words and the set's checks (pure). The route
composes `AgentHeader` and passes `useRefreshSetupProgress` as `onChange`.

## Shape and direction

```
features/<name>/
  index.ts        public API: the only file other folders may import
  views/          React components. No fetching, no generated-client imports.
  state/          queryOptions and hooks, XState machines, Zustand stores. No JSX.
  api/            wraps shared/api/generated; maps DTOs ↔ view models. No React.
  __contract__/   failing-first contract tests for this feature (tag-pinned)
```

`views → state → api → shared/api`. Every layer may use `shared/ui` and `shared/lib`.
dependency-cruiser rules: `no-cross-feature`, `feature-public-api-only`, `views-not-to-api`,
`state-not-to-views`, `api-not-to-state-or-views`, `generated-client-only-from-feature-api`,
`features-not-to-app-or-routes`, `base-ui-only-in-shared-ui`, `no-circular`.

**Types for views:** views may not import `api/`, even for types. A feature re-exports the view
models, contract limits and pure error readers its views need from `state/` (agents:
`state/models.ts`). Request rules come from the generated schemas (agents: `api/request-checks.ts`),
never from hand-copied numbers.

Two features never import each other. If both need something, move it to `shared/` or compose
them in a route.

**Plans** (`plans/`): `PlanPage` loads the plan; `PlanEditor` (`state/use-plan-editor.ts`)
holds the draft and autosaves it 500 ms after the last edit, or on "Use this plan" while only
the template exists. The canvas lays steps out in columns by distance from the start
(`state/layout.ts`, pure) and draws paths as SVG; every step is a named group with its paths
in words, its problems, and its own Edit, Add path and Delete actions, so the whole editor
works by keyboard. No graph library: plans are small (≤ 50 steps), and a fixed scale keeps
every target at 44 px for `auditScreen`. The agents feature exports `AgentHeader` and
`useRefreshSetupProgress` for the Plan route.

## Conventions by layer

**`views/`**: one component per file, ≤ 200 lines, ≤ 8 props. Build from `shared/ui` only; no raw
`<button>` (lint). **One primary action per screen** (`<Button variant="primary">`). Targets
≥ 44 px come from `shared/ui`. Show `aria-busy` or a skeleton within 100 ms of any input
(Doherty). Copy is short, specific and in sentence case.

**`state/`**: export `queryOptions(...)` factories and thin hooks around them. Use XState for
flows with real states (publish). The M4 test call is a pure reducer
(`testing/state/test-call.ts`) behind a thin hook; it moves to XState if the M5 Test step
outgrows it. Use Zustand only for small client-side prefs.
Optimistic updates for edits; autosave ≤ 1 s after the last change (Parkinson).

**`api/`**: the only place that imports `shared/api/generated`. Convert DTOs to view models
here, so a contract change touches one file per feature.

**`__contract__/`**: contract tests written before the feature, registered in
`contracts/contracts.toml` and pinned by `contract/<feature>/vN`. Never edit after tagging.

## Testing and coverage

- `state/`: unit tests for every query option, machine transition and store action.
  **Coverage floor 90 %.**
- `api/`: unit tests with MSW handlers generated by orval.
- `state/` and `views/` may not import the generated client or its mocks; their tests use the
  in-memory API fakes in `src/test/agents-api.ts`, `src/test/plans-api.ts`,
  `src/test/voice-api.ts` and `src/test/test-runs-api.ts` (plain MSW), `src/test/msw-server.ts` and `src/test/render-routes.tsx`
  (views in a memory router). Test call views run against `src/test/fake-voice-browser.ts`
  (fake socket, microphone, Web Audio).
- `views/`: Testing Library by role and name, plus an E2E screen with `auditScreen`.
- Feature totals count toward the 80 % frontend floor.

## Example

```ts
// features/agents/state/agents-queries.ts
import { queryOptions } from '@tanstack/react-query';

import { listAgents } from '../api/agents-api';

export const agentsQueryOptions = () =>
  queryOptions({ queryKey: agentKeys.list(), queryFn: listAgents });

// features/agents/index.ts: the views routes compose
export { AgentsOverview } from './views/agents-overview';
export { CreateAgentPage } from './views/create-agent-page';
export { ProvisionPage } from './views/provision-page';
```
