# Voice Agent Kitchen — Project Brief

## 1. Goal

Build **Voice Agent Kitchen**, a platform where I create, test, and publish AI voice agents that do whatever task I configure them for: outbound telemarketing, customer support, customer service, appointment booking, surveys, and more.

The core flow must be simple:

1. **Provision**: create an agent with a name, purpose, persona, voice, language, model, and tools/integrations.
2. **Plan**: define what the agent does: goals, conversation flow or script, knowledge, guardrails, and escalation/handoff rules.
3. **Test**: talk to the agent in the browser, run simulated conversations and scripted test scenarios, and review transcripts and results.
4. **Publish**: when I'm happy, release an immutable version to a channel (phone number, web widget, API). I can roll back at any time.

## 2. Scope of this step

Research, plan, and build the foundation. **Do not build product features yet.**

Deliverables:

1. **Research report** (`docs/research/`):
   - Voice pipeline design: cascaded STT → LLM → TTS vs. speech-to-speech; latency budget; interruptions (barge-in); turn detection.
   - Databricks fit:
     - Databricks Apps: running a Rust binary, WebSockets, inbound traffic from the phone carrier, auth
     - Model Serving: LLM, STT and TTS latency for real-time voice
     - Lakebase: connecting from Rust
   - Local equivalents for development.
   - TypeSafe AI's Jev "System One" models on Databricks (typed, fast decisions instead of generated text, e.g. `ai_decide` and `openjev-qwen35-4b`): where they fit in the agent (routing, detection, scoring, test evaluation).
   - NVIDIA Nemotron 3.5 Bolt models (E4B, 74B-A9B): fit as the main voice LLM, a fast first responder, the local-dev model, or the simulated caller in tests.
   - Telephony (PSTN/SIP) and WebRTC options.
   - Rust ecosystem choices: web framework, async runtime, database access and migrations, real-time audio transport, observability.
   - Frontend stack recommendation.
   - Compliance for outbound calling and recording: consent, do-not-call lists, AI disclosure, call-recording laws, data privacy.
   - Comparable products, so the UX follows patterns users already know.
2. **Architecture and implementation plan** (`docs/plan/`): components, data model, API contracts, milestones, and build order.
3. **Repository skeleton**: folders for every layer and module, each with its own `CLAUDE.md`.
4. **Quality foundation**: the verify contract, CI gates, GitHub settings, Claude Code hooks, reviewer subagents, and Graphify.

For decisions that are hard to reverse (models, telephony carrier, frontend framework), give me a recommendation with tradeoffs and wait for my approval.

## 3. Tech constraints

- **Backend: 100% Rust**, optimized for low latency and efficiency. Use a Cargo workspace with separate crates per layer/module.
- **Frontend**: separate from the backend and highly modular. Stack to be chosen from the research.
- **Production runs entirely on Databricks:**
  - Hosting: **Databricks Apps**
  - LLM, speech-to-text (STT) and text-to-speech (TTS): **Databricks Model Serving**
  - Database: **Lakebase** (Databricks-managed Postgres)
- **Local development** uses local models (LLM, STT, TTS) and a local Postgres behind the same interfaces. Switching between local and Databricks is configuration only, with no code changes.
- Telephony (phone numbers and calls) is not a Databricks service. The carrier is still to be chosen.
- The whole system runs locally with one command (for example, Docker Compose).

## 4. Architecture

### Backend: layered, strict dependency direction

```
routers       HTTP/WebSocket handlers, request validation, DTOs
   ↓
services      business logic and orchestration
   ↓
repositories  persistence
   ↓
domain        pure types and rules, no I/O
```

- **Adapters** for external providers (STT, TTS, LLM, telephony) sit behind traits, so providers can be swapped and mocked in tests. Each of STT, TTS and LLM has a local adapter (dev) and a Databricks Model Serving adapter (prod).
- Layers depend only downward. Enforce this with crate boundaries so violations fail to compile.
- The domain layer has no framework or I/O dependencies.
- The real-time voice pipeline is its own module with its own latency budget.

### Frontend: modular

- Feature modules that match the flow: agents, plans, testing, publishing.
- Shared UI components and design tokens.
- Same layering as the backend: views → state → API client.
- API client generated from the backend contract (OpenAPI), so the two sides can't drift apart.

### `CLAUDE.md` in every folder

Each top-level and layer folder gets a short `CLAUDE.md` stating:

- what the folder is responsible for
- what it may and may not depend on
- conventions and best practices for that layer
- testing expectations
- a small example

## 5. Development process

### Test-driven, contract-first

- For every feature, write the tests **first**. They are the contract.
- Commit the failing contract tests and tag that commit (for example, `contract/<feature>/v1`).
- The implementation must make those tests pass **without editing them**. CI fails if contract test files differ from their tag. Changing a contract requires a new version tag and my approval.
- Test levels:
  - **Unit**: every layer, mocking the layer below.
  - **Integration**: repositories against a real database in a container; routers over HTTP.
  - **Contract**: API schemas shared by frontend and backend.
  - **End-to-end**: Playwright for the UI; simulated calls for voice agents.

### Worktrees and branches

- Every unit of work happens in its own **git worktree** on its own branch.
- Changes reach `main` only through a pull request, only when **all CI checks are green**, and through the **merge queue**.
- No direct pushes to `main`.

## 6. Verify contract

One command (for example, `just verify`) runs the same checks locally, in hooks, and in CI. Any failure blocks the merge.

| Check    | Rust                                                          | Frontend (if TypeScript)        |
| -------- | ------------------------------------------------------------- | ------------------------------- |
| Format   | `rustfmt`                                                     | Prettier or Biome               |
| Lint     | `clippy` with warnings as errors                              | ESLint or Biome                 |
| Types    | `cargo check`                                                 | `tsc --noEmit`, strict mode     |
| Tests    | `cargo nextest`                                               | Vitest, Playwright              |
| Coverage | `cargo-llvm-cov` with a minimum threshold (e.g. 80%)          | Vitest coverage, same threshold |
| Security | `cargo-audit` (CVEs), `cargo-deny` (advisories, licenses, banned crates, sources) | `npm audit` or `osv-scanner` |
| Secrets  | `gitleaks`                                                    | `gitleaks`                      |

## 7. GitHub gates

- **Dependabot**: version and security updates for Cargo, npm, GitHub Actions, and Docker.
- **Dependency review** on every PR: block new vulnerable dependencies and disallowed licenses.
- **CodeQL** code scanning for Rust and JavaScript/TypeScript.
- **Secret scanning** with push protection.
- **Rulesets** on `main`: require PRs, required status checks, up-to-date branches, linear history, no force pushes or deletion.
- **CODEOWNERS** for each layer and area.
- **Merge queue**.
- Workflow hardening: pin actions to commit SHAs, least-privilege permissions.

## 8. Claude Code hooks and reviewer subagents

Claude Code hooks run gate checks automatically on every code change. Each gate is a dedicated **subagent** that reviews the change and returns pass/fail with findings. A change isn't done until every relevant gate passes.

Hooks:

- **After every file edit**: format and fast lint on the changed files; update the Graphify graph.
- **Before a task finishes**: run `verify` and the relevant reviewer subagents.
- **Git pre-commit and pre-push**: the same checks, so they also apply outside Claude.

### Engineering reviewers

| Reviewer      | Checks                                                                 |
| ------------- | ---------------------------------------------------------------------- |
| Performance   | latency in the voice path, allocations, blocking calls in async code, N+1 queries |
| Security      | authentication and authorization, input validation, secrets, injection, PII in logs, CVEs |
| Hygiene       | dead code, leftover TODOs, naming, unused dependencies                  |
| CI/CD         | workflow correctness, pinned actions, permissions, caching             |
| Code quality  | readability, complexity, duplication, error handling                   |
| Separation    | layer boundaries and dependency direction                              |
| Testing       | tests written first, contract tags untouched, coverage, meaningful assertions |
| Dependencies  | necessity, licenses, maintenance status, duplicates                    |
| Docs          | `CLAUDE.md` files, README, API docs, architecture decision records     |

### UX reviewers (run on frontend changes)

One subagent per group. Each checks its laws against a written checklist in `docs/ux-guardrails/` with concrete, measurable rules where possible.

| Group                          | Laws and what they mean here                                                                  |
| ------------------------------ | --------------------------------------------------------------------------------------------- |
| Cognitive load and decisions   | **Hick's law**: few choices per step. **Miller's law**: chunk information (about 7 ± 2 items). **Occam's razor**: the simplest design that works. **Pareto principle**: polish the 20% of features that drive 80% of use. |
| Interaction and ergonomics     | **Fitts's law**: large targets (at least 44×44 px), placed close to where the user is working; minimize target distance. **Doherty threshold**: respond within 400 ms, or show immediate feedback. |
| Visual perception and layout   | **Jakob's law**: follow patterns users know. **Proximity**, **similarity**, **uniform connectedness**: group related things visually. **Prägnanz**: simple, clear shapes. **Von Restorff effect**: the one key action stands out. |
| Memory, experience, and timing | **Serial position effect**: most important items first and last. **Peak-end rule**: design the high point and the ending (the first successful test call, publishing). **Zeigarnik effect**: show progress on unfinished setup. |
| System complexity and behavior | **Tesler's law**: the system absorbs complexity, not the user. **Postel's law**: accept flexible input, produce strict output. **Parkinson's law**: keep tasks short with smart defaults and autosave. |

## 9. Code knowledge graph (Graphify)

- Use **Graphify** to build a knowledge graph of the entire codebase.
- Keep it current automatically with hooks on every code change (Claude Code post-edit hook and git post-commit hook).
- Agents use the graph to navigate the code and to check architecture rules, such as layer violations.

## 10. Definition of done

A change is done when:

- its contract tests were written first, are untouched, and pass
- `verify` passes locally and CI is green
- every relevant reviewer subagent passes
- the graph and docs are updated
- it merged through the merge queue from its worktree branch
