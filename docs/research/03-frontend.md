# 03 — Frontend stack, modularity enforcement and automatable UX guardrails

Research date: **2026-10-30**. Versions come from the npm registry, crates.io and GitHub APIs on that date, unless a row says otherwise.
Status: **recommendation, waiting for approval.** The frontend framework is a hard-to-reverse decision under PROMPT.md §2.

---

## TL;DR

- **Framework: React 19.3 + Vite 8 SPA, TypeScript 7, served as static files.** It's the only option that has all three of the most mature flow editor (React Flow 12.12), first-party React SDKs from every major voice/WebRTC vendor, and the deepest test tooling. Rust frontends (Leptos 0.8, Dioxus 0.7, Yew 0.23) are still pre-1.0, have no production-grade flow editor, and would need JS glue for audio anyway.
- **UI: shadcn/ui (CLI 4.23) on Base UI + Tailwind 4.3**, with tokens in DTCG JSON compiled to Tailwind `@theme` variables. Since July 2026, shadcn defaults to Base UI. React Flow UI and shadcn's new chat/transcript components plug straight in.
- **Data/routing: TanStack Query 5 + TanStack Router 1** (typed, schema-validated URL state). Use **XState 5** for the voice-session state machine and **Zustand 5** for the little client state that's left.
- **Contract: Rust emits `openapi.json` → orval 8 generates fetch client + Query hooks + Zod + MSW mocks.** CI gates: spec drift (`git diff`), `oasdiff breaking`, regenerated client drift, `tsc`.
- **Structure: "FSD-lite" feature folders** (`features/{agents,plans,testing,publishing}/{views,state,api}`). **dependency-cruiser** enforces the boundaries as the single source of truth.
- **The toolchain moved a lot in 2026.** TypeScript 7 (native Go, July 2026) has **no compiler API**, so typescript-eslint is blocked. **Oxlint + tsgolint** (type-aware, stable July 2026) + Oxfmt fill that gap. Vitest 5, Playwright 1.64, Storybook 10 and axe-core 4.14 make up the rest. Of the UX laws, **12 of 18 get hard CI gates and the other 6 get heuristic gates plus reviewer checklists**. The gates run through a shared Playwright `auditScreen()` helper, axe, lint rules and perf budgets.

---

## 1. Recommendations

| # | Decision | Recommendation | Confidence | Why, in one line |
|---|---|---|---|---|
| R1 | Framework | **React 19.3 + Vite 8 SPA** (no SSR) | **High** | Flow editor, voice SDKs and test tooling are all React-first. The dashboard is behind auth, so SEO doesn't matter. |
| R2 | Rust frontend | **No** for the app shell. *Optional later:* compile the Rust `domain` plan validator to WASM so browser and server share the same rules | High (no) / Low (WASM idea) | Pre-1.0 frameworks, no flow editor, AudioWorklet still has to be JS |
| R3 | UI kit | **shadcn/ui on Base UI**, Tailwind 4.3, lucide icons | Medium-high | You own the code, it's accessible, and React Flow UI and AI chat/transcript components plug in |
| R4 | Tokens | DTCG JSON → Style Dictionary 5 → Tailwind `@theme` (default theme reset) | Medium | Off-token classes then don't exist at all, which is enforcement by construction |
| R5 | Server state | **TanStack Query 5.105** | High | De-facto standard. Optimistic updates help with Doherty. |
| R6 | Routing | **TanStack Router 1.170** (file-based, `validateSearch`) | Medium-high | Typed params and search params with schema fallback, which is Postel's law for URLs |
| R7 | Client state | **XState 5** for voice session and publish flow; **Zustand 5** for UI prefs | Medium | Explicit states make the Test step testable. Zustand stays tiny for everything else. |
| R8 | API client | **orval 8.45** (fetch + react-query + zod + msw) | Medium | Stable major, very active, and the only generator with first-class MSW + Faker mocks |
| R9 | Contract CI | Spec-drift check + **oasdiff 1.35** `breaking` + client-drift check + `tsc` | High | Two-sided drift detection. Breaking changes need an approval label. |
| R10 | Structure | **FSD-lite feature folders** + per-feature `views → state → api` | Medium-high | Matches PROMPT.md §4 exactly. Full FSD (7 layers) is overhead for 4 features. |
| R11 | Boundaries | **dependency-cruiser 18.7** (gate) | High | Doesn't care which linter you use, also catches cycles and orphans, and exports a JSON graph that can feed Graphify |
| R12 | Lint/format | **Oxlint 1.90 `--type-aware` (tsgolint 7) + Oxfmt** | Medium | The only stable type-aware lint that works on TS 7 today. Fallback: Biome 2.6. |
| R13 | Types | **TypeScript 7.0.2** `tsc --noEmit`, strict+ | High | 8–12× faster type-checks fit post-edit hooks |
| R14 | Tests | Vitest 5 (+ Browser Mode for canvas/audio components), Testing Library, MSW 3, Playwright 1.64 | High | Covers unit → component → E2E, including a fake mic |
| R15 | Stories/visual | Storybook 10.6 + addon-vitest + addon-a11y; visual diffs with Playwright `toHaveScreenshot` in Docker | Medium | Stories double as interaction and a11y tests, and visual diffs stay self-hosted and free |
| R16 | Perf budgets | Playwright Event-Timing (INP) gate + `size-limit`; Lighthouse 13 user-flow nightly only | Medium | `@lhci/cli` hasn't had a release since June 2025 and still pins Lighthouse 12 |
| R17 | Package manager | **pnpm** pinned via `packageManager`, `minimumReleaseAge` set (supply-chain delay) | High (pnpm) / Medium (v12) | pnpm 12 is a Rust rewrite (Aug 2026). Pin 11 if you want maximum conservatism. |

### R1 in more detail: why React and not the alternatives

- **Flow editor (Plan step).** `@xyflow/react` 12.12 is MIT-licensed, ships releases monthly, and has 38.8k★. It also has **React Flow UI**, a set of shadcn-based node and edge components ([reactflow.dev/ui](https://reactflow.dev/ui)). Svelte Flow 1.7 is the only comparable alternative. Solid has no official equivalent. For Dioxus there's only `dioxus-flow` 0.1.1, and for Leptos there's nothing.
- **Voice in the browser (Test step).** LiveKit (`livekit-client` 2.23 + `@livekit/components-react`), Pipecat (`client-react` 1.11 + `voice-ui-kit`), ElevenLabs (`@elevenlabs/react`) and Daily all ship React bindings first. Whatever transport the backend picks (WebRTC SFU or WebSocket PCM), the client side is ready-made in React.
- **Tests.** Testing Library, Storybook 10, Playwright component and E2E tests, MSW and Vitest Browser Mode are all React-first.
- **TS 7 compatibility.** Svelte, Vue and Astro editor tooling still needs TS 6 because TS 7.0 has no API ([TS 7 announcement](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/)). React/TSX works natively.
- **Next.js 16.4 is not worth it here.** RSC and a Node server add a second backend runtime next to Rust, plus an extra security surface: 16.2.6 alone patched 13 advisories. The frontend is behind auth, so SEO and SSR buy nothing.
- **What Rust frontends would give you, honestly:** one language, and shared types with the backend. But the shared-types benefit is already covered by OpenAPI codegen. The costs are concrete: pre-1.0 churn (Leptos 0.9 is in RC, Dioxus 0.8 in alpha), no accessible-primitive library at the level of Base UI or Radix (`dioxus-components` has no releases yet), no Storybook, and much slower UI iteration because of compile times. Mic capture still needs a JS AudioWorklet. Revisit in about 12 months, or use WASM only for shared domain rules (R2).

---

## 2. Comparison tables

### 2.1 Frontend frameworks (status 2026-10-30)

| Option | Version | Flow editor | Voice/WebRTC client SDKs | Test tooling | Familiarity (people + LLMs) | Verdict |
|---|---|---|---|---|---|---|
| **React + Vite SPA** | react 19.3.1, vite 8.3.5 | React Flow 12.12 + React Flow UI | LiveKit, Pipecat, ElevenLabs, Daily (all React) | Best in class | Highest | **Pick** |
| Next.js | 16.4.3 | Same as React | Same | Same + RSC test gaps | High | Extra Node server, no benefit behind auth |
| SvelteKit + Svelte 5 | kit 3.1.1, svelte 5.57 | Svelte Flow 1.7 | Vanilla JS SDKs only (no official Svelte kits) | Good (Vitest, Playwright) | Medium | Runner-up; editor tooling stuck on TS 6 |
| SolidStart | 2.1.0 (solid 1.9) | None official | Vanilla JS only | OK | Low | Ecosystem too small |
| Leptos | 0.8.22 (0.9.0-rc0) | None | `web-sys` + hand-written bindings | `wasm-bindgen-test`, Playwright | Low | Not yet |
| Dioxus | 0.7.10 (0.8 alpha) | `dioxus-flow` 0.1.1 (early) | Hand-written bindings | Limited | Low | Not yet |
| Yew | 0.23.0 (Mar 2026) | None | Hand-written bindings | Limited | Low | Slower momentum |

### 2.2 UI kit

| Option | Version | A11y primitives | Fits flow editor / transcript | Bundle and styling | Risk |
|---|---|---|---|---|---|
| **shadcn/ui + Base UI** | shadcn 4.23, @base-ui/react 1.9 | Base UI (MUI-backed, 1.0 since Dec 2025) | React Flow UI; shadcn Message, Bubble, Trace, Code Block, Message Scroller (Jun–Oct 2026) | Copy-in, Tailwind 4; you own the code | You maintain the copied code |
| shadcn/ui + Radix | radix-ui 1.8.1 | Radix (WorkOS; cadence slowed, 258 open issues+PRs) | Same | Same | Upstream momentum |
| Mantine | 9.7.4 | Own | Needs adapters | CSS modules, batteries included | Second design language next to React Flow UI |
| MUI | 9.5.0 | Own / Base UI | Needs adapters | Emotion runtime, Material look | Heavier and less customisable |

### 2.3 Routing and state

| Concern | Option A | Option B | Pick |
|---|---|---|---|
| Router | **TanStack Router 1.170**: typed routes and search params, `validateSearch` with Zod/Valibot, loaders integrate with Query | React Router 8.5: non-breaking from v7; full type safety mainly in framework mode; Node 22+, ESM-only | A |
| Server state | **TanStack Query 5.105** (v6 not released) | SWR | A |
| Client state | **Zustand 5.0** (stores) | Jotai 3.0 (atoms, new major Sep 2026) | A |
| Complex flows | **XState 5.33** for voice session (`idle → connecting → listening ⇄ agentSpeaking → bargeIn → ended/error`) and publish/rollback | Hand-rolled reducers | XState |
| Forms | TanStack Form 1.33 or react-hook-form 7.91 + Zod 4.6 | — | RHF + Zod (most familiar); revisit TanStack Form |

### 2.4 OpenAPI → TypeScript client

| | **orval** | hey-api/openapi-ts | openapi-typescript + openapi-fetch |
|---|---|---|---|
| Version / last release | 8.45.0 / 2026-10-25 | 0.99.0 / 2026-06-22 (`next` builds to 2026-09-30) | 7.13.0 + 0.17.0 / 2026-02-11 |
| Maturity | Stable major (v8 since Jan 2026), 19 open issues+PRs | Pre-1.0, 661 open issues+PRs, date-tagged releases | Stable, minimal, slow cadence |
| Output | Fetch/axios client, TanStack Query hooks and `queryOptions`, Zod, **MSW handlers + Faker** | SDK, TanStack Query, Zod/Valibot plugins; MSW plugin was an open PR (#3570), not confirmed shipped | Types only + ~6 kB typed fetch; `openapi-react-query` wrapper |
| TDD fit | Mocks come from the contract for free | Mocks are hand-written or third-party | Mocks are hand-written (`openapi-msw`) |
| Pick | **Yes** | Strong alternative once 1.0 ships | Choose if you want zero generated runtime |

### 2.5 Boundary enforcement

| Tool | Version | What it checks | Speed / where | Notes |
|---|---|---|---|---|
| **dependency-cruiser** | 18.7.0 | Path-based allow/forbid rules, cycles, orphans, `index.ts`-only entry | ~1–2 s, CLI; pre-push + CI | **Gate.** JSON/dot graph export. Under TS 7 use the `swc` parser or a TS 6 alias (verify). |
| eslint-plugin-boundaries | 7.4.0 | Element/type policies, in-editor squiggles | Runs under ESLint **or Oxlint `jsPlugins` (alpha)** | Optional editor feedback layer |
| Steiger (FSD) | 0.8.1 | FSD-specific rules (`insignificant-slice`, …) | CLI | Only useful with full FSD; still 0.x |
| Biome `noRestrictedImports` / `noPrivateImports` | 2.6.4 | Import bans per override; `@package`/`@private` JSDoc visibility | Instant | Good approximation if you pick Biome |
| Nx `enforce-module-boundaries` | nx 24.0 | Tag-based project rules | Needs an Nx workspace | Overkill for one app |
| Turborepo boundaries | turbo 2.11 | Package-level tags | — | Still **experimental**, package-level only |

### 2.6 Lint and format (TS 7 era)

| Stack | Type-aware lint on TS 7 | Speed | Plugins / custom rules | Maturity | Verdict |
|---|---|---|---|---|---|
| **Oxlint 1.90 + tsgolint 7 + Oxfmt 0.75** | **Yes**, native: 59/61 typescript-eslint typed rules (stable 2026-07-22) | Fastest | ESLint-compatible JS plugins (alpha); native react, jsx-a11y, import, vitest | Oxlint stable; Oxfmt pre-1.0 | **Pick** |
| Biome 2.6.4 | Own inference; `noFloatingPromises` still nursery | Very fast, one binary | GritQL + JS plugins | Stable | Fallback (simplest) |
| ESLint 10 + typescript-eslint 8.72 + Prettier 3.9 | Only with `typescript` aliased to `@typescript/typescript6` | Slow | Largest ecosystem | Stable | Only if a must-have plugin forces it |

**Vite+ 1.0** (Sep 28 2026, MIT; VoidZero is now part of Cloudflare) bundles Vite 8, Vitest 5, Oxlint, Oxfmt, tsgolint and Rolldown behind one `vp` CLI, including `vp hooks` and `vp staged`. Adopt the same tools individually now. Optionally switch to `vp` later, since `just verify` already provides the single command.

### 2.7 Test and quality pyramid (frontend half of the verify contract)

| Level | Tool | Gate |
|---|---|---|
| Format | Oxfmt `--check` | Post-edit hook (changed files), verify |
| Lint | Oxlint `--type-aware` (+ jsx-a11y, react-hooks, vitest rules) | Post-edit (changed files), verify |
| Types | `tsc --noEmit` (TS 7, strict + `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `verbatimModuleSyntax`) | Verify |
| Boundaries | `depcruise src --config` | Pre-push, verify |
| Unit | Vitest 5 (Node env) for `state/`, `api/`, `lib/` | Verify, **80 % coverage** (v8) |
| Component | Vitest 5 Browser Mode (Chromium via Playwright) + Testing Library + MSW 3 | Verify |
| Stories | Storybook 10 `addon-vitest` (play functions) + `addon-a11y` (`test: 'error'`) | Verify |
| E2E | Playwright 1.64 against `docker compose` stack or MSW-mocked build; fake mic via `--use-file-for-fake-audio-capture` | CI (sharded), pre-push smoke |
| A11y | `@axe-core/playwright` 4.14 with tags `wcag2a, wcag2aa, wcag21aa, wcag22aa` | Every E2E screen |
| Visual | Playwright `toHaveScreenshot` on Storybook stories, in a pinned Docker image | CI |
| Perf | Playwright Event-Timing INP gate, `size-limit` JS budget | CI |
| Dead code/deps | Knip 6.44 | Verify (Hygiene reviewer) |
| Security | `pnpm audit` / `osv-scanner`, gitleaks | Verify |

---

## 3. Proposed frontend folder tree

```
contracts/
  openapi.json                 # emitted by Rust (utoipa/aide), committed; source of truth
frontend/
  CLAUDE.md
  package.json  pnpm-lock.yaml  pnpm-workspace.yaml   # packageManager pinned, minimumReleaseAge
  vite.config.ts  vitest.config.ts  playwright.config.ts  orval.config.ts
  .oxlintrc.json  .oxfmtrc.json  .dependency-cruiser.cjs  tsconfig.json
  .size-limit.json
  tokens/                      # DTCG *.tokens.json → style-dictionary → src/shared/ui/tokens.css
  src/
    app/                       # CLAUDE.md — bootstrap, providers (QueryClient, Router), global CSS, error boundary
    routes/                    # CLAUDE.md — TanStack file routes; THIN: compose feature views, validateSearch
      __root.tsx  agents/  agents/$agentId/{provision,plan,test,publish}.tsx
    features/                  # CLAUDE.md — one folder per flow step; no cross-feature imports
      agents/                  # Provision
        index.ts               # public API — the only importable surface
        views/                 # CLAUDE.md — React components; no fetch, no generated imports
        state/                 # CLAUDE.md — queryOptions/hooks, Zustand stores, XState machines; no JSX
        api/                   # CLAUDE.md — wraps generated client; DTO ↔ view-model mapping
        __contract__/          # failing-first contract tests (tag-checked in CI)
      plans/                   # Plan: flow editor (React Flow), guardrails, knowledge
      testing/                 # Test: live call, transcript, scenarios (XState voice-session machine)
      publishing/              # Publish: versions, channels, rollback
    shared/
      api/
        generated/             # orval output — never hand-edited, lint-ignored, drift-checked
        http.ts                # fetch mutator: auth, error normalisation, Zod response parse
      ui/                      # CLAUDE.md — shadcn components (owned), Button/Card/…; only place importing @base-ui/react
        tokens.css             # generated Tailwind 4 @theme (default theme reset)
      voice/                   # CLAUDE.md — VoiceSession port + adapters (webrtc|websocket|fake), AudioWorklet
      lib/                     # pure helpers (format, phone normalisation, debounce)
    test/
      msw/                     # generated handlers + scenario overrides
      fixtures/  audio/        # e.g. hello-agent.wav for the fake mic
  e2e/
    golden/                    # @golden paths: create agent, edit plan, test call, publish, rollback
    support/ux-audit.ts        # auditScreen(): UX-law assertions (see §4)
  .storybook/
```

**Allowed dependency direction** (encoded in dependency-cruiser):
`app → routes → features/*/index.ts`; inside a feature, `views → state → api → shared/api`. Every layer may use `shared/{ui,lib}`.
Forbidden:
- `features/A → features/B`
- `views → api` or `views → shared/api/generated`
- `state → views`
- `shared → features`
- anything except `shared/ui` importing `@base-ui/react`
- any cycle

---

## 4. UX-law rules (automatable guardrails)

**Legend:**
- **A**: automated gate (fails CI).
- **P**: partially automated (a heuristic gate plus a reviewer).
- **R**: AI-reviewer checklist only.

**Shared mechanisms:**
- `auditScreen(page)` runs on every E2E screen.
- `data-variant` on `<Button>`.
- Oxlint custom JS-plugin rules in `tools/lint-rules/`.
- Token-only Tailwind theme.

> The brief says "19 laws" but lists **18**. All 18 are covered below. If a 19th was intended, *common region* fits the "Visual perception" group (see Open questions).

| Law | Measurable rule(s) | How checked |
|---|---|---|
| **Hick** | ≤ 1 primary + ≤ 3 secondary visible actions per view region; extra actions go into a "More" menu. Any choice list with > 7 options must be a searchable Combobox. | **A**: `auditScreen` counts `[data-variant=primary]:visible` (≤1) and buttons per `[data-region=actions]` (≤4). Lint rule flags `<Select>` with > 7 static options. |
| **Miller** | Primary nav ≤ 7 items. ≤ 7 inputs per `fieldset`/section. Tables show ≤ 7 columns by default. Phone numbers and IDs display chunked (`+1 415 555 0100`). | **A**: `getByRole('navigation',{name:'Primary'}).getByRole('link')` count ≤ 7; inputs per fieldset ≤ 7; unit tests on formatters. |
| **Occam** | Component file ≤ 200 lines; cyclomatic complexity ≤ 10; ≤ 8 props; no new dependency if `shared/` already covers it; no unused exports. | **P**: Oxlint `max-lines` / `complexity`; Knip; Dependencies reviewer judges "simplest design". |
| **Pareto** | The 5 golden paths (create agent, edit plan, test call, publish, rollback) each have an `@golden` E2E, a perf budget and visual snapshots. Every primary step emits an analytics event. | **P**: CI script fails if any path listed in `docs/ux-guardrails/golden-paths.md` lacks a `@golden` spec. Reviewer decides where polish goes. |
| **Fitts** | Interactive targets ≥ **44×44 CSS px** (inline text links exempt). Commit button sits inside the form/panel it commits. Node actions sit on the node toolbar, not a global toolbar. | **A**: axe `target-size` (WCAG 2.2 AA, 24 px floor); `auditScreen` measures `getBoundingClientRect()` ≥ 44 on `button, [role=button], [role=tab], input[type=checkbox|radio]`; asserts `form button[type=submit]` exists. Placement is **R**. |
| **Doherty** | Visual feedback ≤ 100 ms after any input. Result or progress indicator ≤ **400 ms**. Lab INP ≤ 200 ms on golden paths at 4× CPU throttle. Initial JS ≤ 200 kB gz. | **A**: Playwright Event-Timing gate (below). MSW `delay(1000)` test asserts `aria-busy`/skeleton appears ≤ 100 ms. `size-limit`. Lighthouse 13 user-flow nightly (TBT/INP trend). |
| **Jakob** | Only `shared/ui` primitives (no custom dropdowns/dialogs). Standard shell: left sidebar + top bar + Cmd/Ctrl-K palette. Esc closes overlays, Enter submits. Terminology matches comparable products (see `docs/research/` comparables). | **P**: dependency-cruiser bans `@base-ui/react` outside `shared/ui`; lint bans raw `<select>`/`<dialog>` in features; Playwright keyboard tests (Esc, Cmd-K). Pattern fit is **R**. |
| **Proximity** | Spacing comes only from the token scale (4-pt). Gap inside a group < gap between groups (e.g. ≤ 8 px vs ≥ 24 px). | **P**: Tailwind default theme reset (non-token classes don't exist); lint bans arbitrary values (`-[`), inline `style` margin/padding; visual snapshots. Grouping is **R**. |
| **Similarity** | One `Button` with fixed CVA variants. Status uses semantic tokens only (`success/warning/danger`). One icon set (lucide). Same entity gets the same badge everywhere. | **A**: lint bans `<button>` outside `shared/ui`, bans non-lucide icon imports and raw hex colours; Storybook visual diffs. |
| **Uniform connectedness** | Related controls share a container with an accessible name (`fieldset`+`legend`, `role=group` + `aria-labelledby`, Card). Flow nodes are linked by visible edges. Stepper steps are joined by a connector. | **P**: `auditScreen` asserts every radio/checkbox set sits in a named group; axe `aria-*` rules. Visual grouping is **R**. |
| **Prägnanz** | ≤ 3 radius tokens, ≤ 3 elevation tokens, ≤ 2 font families, ≤ 6 type sizes, no decorative gradients. | **A**: unit test on the DTCG token files counts tokens per category; lint bans `bg-gradient-*` outside allowlist. Overall clarity is **R**. |
| **Von Restorff** | Exactly one primary CTA when a main action exists. Brand/primary colour is reserved for that CTA and focus states. Destructive actions use the `danger` variant + confirm dialog. | **A**: `[data-variant=primary]` count ≤ 1; lint allows `bg-primary` only in `shared/ui/button`; Playwright: delete/rollback opens `role=alertdialog`. |
| **Serial position** | Primary nav: *Agents* first, *Settings/Account* last. Forms: required fields first, commit action last. Lists default to "recently edited" first. | **A**: Playwright `toMatchAriaSnapshot` on primary nav order; unit test on default sort. Form ordering is **R**. |
| **Peak-end** | First successful test call ends on a summary (turns, latency p50/p95, transcript, "Publish" CTA). Publish ends on version + channel + "Roll back" link + success toast. Every error state ends with a recovery action. | **P**: `@golden` specs assert `role=status` text and the next-step CTA on both end screens; visual snapshot of the end states. Emotional quality is **R**. |
| **Zeigarnik** | Agent shows a 4-step setup checklist (Provision/Plan/Test/Publish) with `role=progressbar` (`aria-valuenow`). Draft badge "Unpublished changes". Progress survives reload. | **A**: Playwright: new agent → progress 0 %; complete Provision → 25 %; reload → unchanged. Unit test on progress calculation. |
| **Tesler** | Create-agent requires ≤ 3 fields (name, purpose, voice); every other field has a server default (voice, language, model, turn-detection timings). One "Advanced" disclosure per form at most. | **A**: test that the OpenAPI `CreateAgent` schema has ≤ 3 `required` and every optional property has a `default`; Playwright creates an agent with only required fields. Disclosure count is **P**. |
| **Postel** | Accept: any phone format (normalise to E.164), trimmed/case-insensitive enums, pasted scripts with mixed line endings, garbage URL params (fall back to defaults). Emit: every request body is Zod-parsed before send; every response is Zod-parsed on receipt. | **A**: property-based tests (fast-check) on normalisers; unit test that the `http.ts` mutator rejects invalid payloads; Playwright visits routes with malformed search params → renders with defaults, no error boundary. |
| **Parkinson** | Autosave drafts ≤ 1 s after the last edit, with a visible "Saved" status (no Save button needed). Create-agent golden path ≤ 8 interactions / ≤ 60 s. Templates for common jobs (support, booking, survey, outbound). | **A**: Playwright: edit → "Saved" ≤ 2 s → reload → value persists; interaction counter on the golden path ≤ 8. Template usefulness is **R**. |

**Tally:**
- Hard CI gate (A): 12 laws (Hick, Miller, Fitts, Doherty, Similarity, Prägnanz, Von Restorff, Serial position, Zeigarnik, Tesler, Postel, Parkinson). Some of these also have reviewer-only items, such as placement or ordering.
- Heuristic gate plus reviewer (P): 6 laws (Occam, Pareto, Jakob, Proximity, Uniform connectedness, Peak-end).
- Reviewer-only: none.

Each UX reviewer subagent reads its group's checklist in `docs/ux-guardrails/` and reruns `auditScreen` results as evidence.

**Sketch: the two core helpers** (illustrative, not final code):

```ts
// e2e/support/ux-audit.ts
export async function auditScreen(page: Page) {
  expect(await page.locator('[data-variant="primary"]:visible').count(), 'Von Restorff/Hick').toBeLessThanOrEqual(1);
  expect(await page.getByRole('navigation', { name: 'Primary' }).getByRole('link').count(), 'Miller').toBeLessThanOrEqual(7);
  const small = await page.locator('button:visible,[role=button]:visible,[role=tab]:visible').evaluateAll(els =>
    els.filter(e => { const r = e.getBoundingClientRect(); return r.width < 44 || r.height < 44; }).map(e => e.outerHTML.slice(0, 80)));
  expect(small, 'Fitts ≥44×44').toEqual([]);
  const axe = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze();
  expect(axe.violations).toEqual([]);
}
// Doherty: Event Timing API (basis of INP), under 4× CPU throttle
await (await page.context().newCDPSession(page)).send('Emulation.setCPUThrottlingRate', { rate: 4 });
await page.addInitScript(() => { (window as any).__ev = [];
  new PerformanceObserver(l => (window as any).__ev.push(...l.getEntries().map(e => e.duration)))
    .observe({ type: 'event', durationThreshold: 16, buffered: true } as PerformanceObserverInit); });
// …run golden path…
expect(await page.evaluate(() => Math.max(0, ...(window as any).__ev))).toBeLessThan(200);
```

---

## 5. Voice in the browser (Test step): frontend side

- **Port and adapters, like the backend.** `shared/voice` exposes a `VoiceSession` interface:
  - methods: `connect`, `mute`, `end`
  - events: `transcript.partial/final`, `agent.state`, `metrics.latency`

  Three adapters sit behind it: `webrtc` (LiveKit or Pipecat SDK, depending on the backend research), `websocket-pcm` (AudioWorklet capture at 16 kHz + playback queue), and `fake` (scripted events for unit tests). Views never touch `getUserMedia`.
- **State.** An XState machine in `features/testing/state` owns the session lifecycle and barge-in. Model-based tests can walk every transition.
- **UI.** The live transcript uses shadcn **Message Scroller / Message / Bubble** (released June 2026). The agent "thinking/tool call" lines use **Trace** (October 2026). Show a live latency badge (Doherty) on every turn.
- **E2E.** Launch Chromium with `--use-fake-ui-for-media-stream --use-fake-device-for-media-stream --use-file-for-fake-audio-capture=e2e/fixtures/audio/hello-agent.wav`. This makes the "talk to the agent" golden path deterministic in CI.

---

## 6. Risks and open questions

| # | Risk / question | Impact | Mitigation |
|---|---|---|---|
| 1 | **TS 7.0 has no compiler API until 7.1.** typescript-eslint refuses TS 7. Some tools that import `typescript` may break, e.g. Storybook's `react-docgen-typescript` and possibly dependency-cruiser's TS parser. | Medium | Oxlint/tsgolint for typed lint. Where a tool needs the API, add `@typescript/typescript6` side-by-side (official guidance). Use Storybook's default `react-docgen`. Verify dependency-cruiser with the `swc` parser in the skeleton spike. |
| 2 | Oxfmt is pre-1.0 (0.75) and Oxlint JS plugins are alpha | Low–Med | Formatting output is Prettier-compatible, so switching to Biome/Prettier later is cheap. The boundary gate is dependency-cruiser, not a JS plugin. |
| 3 | Brand-new majors everywhere: Vitest 5 (Sep), MSW 3 (Sep), pnpm 12 Rust rewrite (Aug), React Router 8, Storybook 11 in alpha | Med | Pin exact versions. Dependabot grouped weekly. `minimumReleaseAge` ≥ 1 day. Node **24 LTS** baseline (Vitest 5 / MSW 3 / RR 8 need ≥ 22). |
| 4 | `@lhci/cli` is stale (last release June 2025, bundles Lighthouse 12.6 vs current 13.6) | Low | Perf gates use Playwright Event Timing + size-limit. Run Lighthouse 13 user-flows nightly, not as a merge gate. |
| 5 | hey-api is still pre-1.0. If you choose orval, regenerated output churns on orval upgrades. | Low | Generated code stays isolated in `shared/api/generated`, and features wrap it, so swapping generators only touches `features/*/api`. |
| 6 | shadcn moved its default to Base UI in July 2026 (reported via the changelog mirror). Radix is still supported, but its momentum is slower. | Low | Start on Base UI. Components are copied into `shared/ui`, so a primitive swap stays local. |
| 7 | Voice transport is undecided (LiveKit/Pipecat WebRTC vs custom WebSocket). It depends on the backend research. | Med | The `VoiceSession` port makes this an adapter choice, not a rewrite. |
| 8 | **The brief says 19 UX laws but lists 18.** | — | Confirm with the user. *Law of common region* is the natural 19th. |
| 9 | Vite+ (`vp`) is new (1.0 on Sep 28 2026), and its owner VoidZero was acquired by Cloudflare | Low | Use the underlying MIT tools directly. Adopting `vp` is optional later. |
| 10 | Not re-verified (search budget ran out): DTCG token spec stable version; whether hey-api's MSW plugin has merged; Mantine 9.6/9.7 changelog details | Low | Check during the skeleton spike |

**Decisions for the user** (hard to reverse first):
1. **React + Vite SPA** vs a Rust frontend (Leptos/Dioxus). Recommendation: React. The WASM shared-validation idea can come later.
2. **Lint/format:** Oxc (Oxlint type-aware + Oxfmt) vs Biome 2.6 (one tool, weaker type-aware rules) vs ESLint + Prettier (needs a TS 6 alias).
3. **API generator:** orval (mocks included) vs hey-api vs openapi-typescript (minimal).
4. **shadcn base:** Base UI (default) vs Radix.
5. **Visual tests:** self-hosted Playwright screenshots (free) vs Chromatic (SaaS). Also whether to adopt Storybook at all. Recommendation: yes.
6. **Common region** as the 19th UX law?

---

## 7. Sources

Versions and dates: npm registry (`registry.npmjs.org/<pkg>`), crates.io API (`crates.io/api/v1/crates/<crate>`), GitHub REST API (repo stats/releases), all queried 2026-10-30.

- Vite 8 announcement: https://vite.dev/blog/announcing-vite8
- TypeScript 7.0 announcement (no API until 7.1; `@typescript/typescript6`): https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/
- typescript-eslint vs TS 7 peer range (secondary): https://codingdunia.com/blog/typescript-eslint-typescript-7-peer-range/
- Oxlint type-aware linting stable (tsgolint v7): https://oxc.rs/blog/2026-07-22-type-aware-linting-stable
- Oxlint JS plugins: https://oxc.rs/docs/guide/usage/linter/js-plugins
- Oxfmt: https://oxc.rs/docs/guide/usage/formatter.html
- Vite+ 1.0 / Cloudflare: https://www.infoq.com/news/2026/10/cloudflare-viteplus-stable/
- Vitest 5.0: https://vitest.dev/blog/vitest-5.html
- React Router 8 (non-breaking, Node 22+, ESM-only): https://reactrouter.com/
- TanStack Router search params: https://tanstack.com/router/latest/docs/framework/react/guide/search-params
- pnpm 12 (Rust rewrite): https://pnpm.io/blog/releases/12.0
- MSW 3.0 release notes (aggregator): https://newreleases.io/project/github/mswjs/msw/release/v3.0.0
- Biome plugins / noFloatingPromises / noPrivateImports: https://biomejs.dev/linter/plugins/ · https://biomejs.dev/linter/rules/no-floating-promises/ · https://biomejs.dev/linter/rules/no-private-imports/
- shadcn/ui changelog (Oct 2026: Trace, Code Block, AI Elements; Base UI/React Aria/Radix): https://ui.shadcn.com/docs/changelog
- shadcn Base UI default (July 2026, mirror of changelog): https://forceui.public.prd.shared.perforce.com/docs/changelog
- Base UI v1 and MUI focus: https://infoq.com/news/2026/02/baseui-v1-accessible/ · https://mui.com/blog/2026-and-beyond/
- Radix momentum (secondary): https://www.pkgpulse.com/guides/shadcn-ui-vs-base-ui-vs-radix-components-2026
- React Flow UI (shadcn-based): https://reactflow.dev/ui
- Svelte Flow 1.x: https://svelteflow.dev/whats-new/2025-10-20
- SolidStart 2.0: https://github.com/solidjs/solid-start/releases/tag/%40solidjs%2Fstart%402.0.0
- Yew 0.22 / 0.23: https://yew.rs/blog/2025/11/29/release-0-22
- Dioxus components (no releases) and dioxus-flow: https://github.com/DioxusLabs/dioxus-components · https://docs.rs/dioxus-flow/0.1.1/src/dioxus_flow/lib.rs.html
- LiveKit React `useVoiceAssistant`: https://docs.livekit.io/reference/components/react/hook/usevoiceassistant
- Pipecat Voice UI Kit: https://docs.pipecat.ai/client/voice-ui-kit
- orval: https://orval.dev/
- hey-api TanStack Query plugin / MSW plugin PR: https://heyapi.dev/openapi-ts/plugins/tanstack-query · https://github.com/hey-api/openapi-ts/pull/3570
- oasdiff: https://github.com/oasdiff/oasdiff
- eslint-plugin-boundaries (JS Boundaries) + Oxlint integration: https://www.jsboundaries.dev/ · https://www.jsboundaries.dev/docs/guides/oxlint-integration/
- Feature-Sliced Design overview / v2.1 "pages first": https://feature-sliced.design/docs/get-started/overview · https://github.com/feature-sliced/documentation/releases/tag/v2.1
- Turborepo boundaries (experimental): https://turbo.build/docs/reference/boundaries
- Storybook 10: https://storybook.js.org/blog/storybook-10/
- axe `target-size` (WCAG 2.2 2.5.8, 24 px): https://dequeuniversity.com/rules/axe/4.10/target-size
- WCAG 2.5.5 Target Size (Enhanced, 44 px): https://www.w3.org/WAI/WCAG22/Understanding/target-size-enhanced.html
- INP: https://web.dev/articles/inp · Lighthouse user flows (timespan INP): https://web.dev/articles/lighthouse-user-flows
- Event Timing API: https://developer.mozilla.org/en-US/docs/Web/API/PerformanceEventTiming
- Lighthouse CI releases (last v0.15.1, 2025-06-26): https://github.com/GoogleChrome/lighthouse-ci/releases
- WebRTC testing with fake media devices: https://webrtc.org/getting-started/testing
