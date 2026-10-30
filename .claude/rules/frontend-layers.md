---
paths:
  - "frontend/**/*.{ts,tsx,css}"
---

# Frontend layers

dependency-cruiser enforces the direction `routes → features → shared`.

- Routes are thin: they compose feature views only.
- A feature is `index.ts` (public API) → `views/` → `state/` → `api/`. No imports between features.
- Views render only: no fetching, no generated-client imports. State holds query options, XState machines and small stores, and has no JSX.
- Only `shared/ui` imports Base UI and builds primitives. Features use `shared/ui` components, design tokens and lucide icons only.
- The backend is reached through `shared/api/generated` (orval) via `shared/api/http.ts`, which Zod-parses requests and responses.
- Never edit generated files: `shared/api/generated/**` (run `just openapi`), `app/routeTree.gen.ts` (from `routes/`) and `shared/ui/tokens.css` (from `frontend/tokens/`).
- UX gates (`docs/ux-guardrails/`): at most one primary action per screen, targets of at least 44×44 px, feedback within 400 ms, token-only spacing and colors.
- Read the folder's `CLAUDE.md` before you edit in it.
