# frontend/

React 19 + Vite 8 single-page app, TypeScript 7 (strict). Built to static files that the Rust
server serves. Read the `CLAUDE.md` of the folder you edit; the rules below apply everywhere.

## Layers

```
app/            bootstrap, providers, router, error boundary
  └─► routes/   thin TanStack file routes; compose feature views
        └─► features/<step>/index.ts   agents · plans · testing · publishing
              views/ ─► state/ ─► api/ ─► shared/api (generated client + http.ts)
every layer ─► shared/ui (components, tokens) · shared/lib (pure helpers)
```

dependency-cruiser (`.dependency-cruiser.cjs`) enforces this; `pnpm depcruise` names the rule
you broke. Never import another feature, never import a feature's internals (only its
`index.ts`), never import `@base-ui/react` outside `src/shared/ui/`.

## Commands

Run from the repo root (recipes `cd` into `frontend/`):

| Command                           | Does                                                                         |
| --------------------------------- | ---------------------------------------------------------------------------- |
| `just frontend-install`           | `pnpm install --frozen-lockfile` and Playwright Chromium                     |
| `just frontend-dev`               | Dev server on :5173, proxies `/api` to :3000                                 |
| `just frontend-verify-fast`       | Format check, lint, types, boundaries                                        |
| `just frontend-verify`            | Plus knip, Vitest + coverage, build, size-limit, Storybook build, Playwright |
| `just frontend-check-file <path>` | Format and lint one file (post-edit hook)                                    |

Inside `frontend/`: `pnpm test`, `pnpm e2e` (after `pnpm build`), `pnpm tokens`, `pnpm storybook`.
Always go through `pnpm`: it runs the Node version pinned in `package.json` (`devEngines.runtime`).

## Toolchain

| Concern    | Tool                                                  | Config                                     |
| ---------- | ----------------------------------------------------- | ------------------------------------------ |
| Format     | Oxfmt (imports and Tailwind classes sorted)           | `.oxfmtrc.json`                            |
| Lint       | Oxlint, type-aware (tsgolint), plus `vak-ux` rules    | `.oxlintrc.json`, `tools/lint-rules/`      |
| Types      | `tsc` 7, strict, `exactOptionalPropertyTypes`         | `tsconfig.{app,node}.json`                 |
| Boundaries | dependency-cruiser (parses with swc: TS 7 has no API) | `.dependency-cruiser.cjs`                  |
| Dead code  | knip                                                  | `knip.config.ts`                           |
| Tests      | Vitest 5 (jsdom + node projects), Playwright 1.64     | `vitest.config.ts`, `playwright.config.ts` |
| Budgets    | size-limit: initial JS ≤ 200 kB gzip                  | `.size-limit.json`                         |
| Tokens     | DTCG JSON → Style Dictionary → Tailwind `@theme`      | `tokens/`                                  |
| API client | orval → `src/shared/api/generated/`                   | `orval.config.ts`                          |

## UX gates

Every screen follows the laws in `docs/ux-guardrails/` (cognitive, interaction, perception,
memory, complexity, golden-paths). The automated parts:

- `auditScreen(page)` in every E2E screen: ≤ 1 visible primary, primary nav ≤ 7, targets
  ≥ 44×44 px, axe WCAG 2.2 AA.
- Lint: no raw `<button>` outside `shared/ui`, no arbitrary Tailwind values, primary colour
  only in `Button`.
- Tokens: the default Tailwind theme is reset, so only token classes exist.

## Never

- Edit generated files: `src/shared/api/generated/**` (`just openapi`),
  `src/app/routeTree.gen.ts` (the router plugin writes it), `src/shared/ui/tokens.css` (`pnpm tokens`).
- Edit a contract test (`e2e/contract/**`, `**/*.contract.test.ts(x)`) once tagged. Bump the version.
- Loosen a gate to make it pass. Fix the cause, or ask.

## Testing and coverage

Tests first. Coverage floors (Vitest, lines): **80 %** overall, **90 %** for
`src/features/*/state/**` and `src/shared/lib/**`. Use Testing Library queries by role and
name, never by class or test ID when a role exists.

## Example

```tsx
// src/routes/agents/index.tsx: a route only composes a feature's public view.
import { createFileRoute } from '@tanstack/react-router';

import { AgentsOverview } from '../../features/agents';

export const Route = createFileRoute('/agents/')({
  staticData: { title: 'Agents' },
  component: AgentsOverview,
});
```
