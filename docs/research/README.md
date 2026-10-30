# Research summary

Researched 2026-10-30. This page combines reports 01–09 into one set of recommendations and the decisions that are still open. Each report has full details and sources.

| # | Report | One-line takeaway |
|---|---|---|
| 01 | [Voice pipeline and models](01-voice-pipeline-providers.md) | Cascaded STT → LLM → TTS on open-weight models; open TTS quality is the biggest compromise |
| 02 | [Rust backend](02-rust-backend.md) | axum + tokio + sqlx + utoipa; layers as crates, enforced three ways |
| 03 | [Frontend](03-frontend.md) | React + Vite SPA; 12 of 18 UX laws get automated CI gates |
| 04 | [Compliance and market](04-compliance-and-market.md) | AI calls are robocalls in law; competitors skip compliance, so it becomes our edge |
| 05 | [Claude Code tooling](05-claude-code-tooling.md) | Hooks enforce gates; reviewers run via `claude -p --agent` from a Stop hook |
| 06 | [Graphify, GitHub, CI](06-graphify-github-ci.md) | Merge queue needs an organization account; Graphify maps code but doesn't enforce rules |
| 07 | [Databricks platform](07-databricks-platform.md) | Feasible except carrier ingress; a small public relay is required |
| 08 | [Jev System One models](08-system-one-models.md) | Fast typed decisions each turn (routing, opt-out, escalation) and test scoring |
| 09 | [Nemotron 3.5 Bolt](09-nemotron-bolt.md) | E4B is a good simulated caller and light local model; not the main LLM |

## What the research changes

1. **"Entirely on Databricks" needs one exception.** Databricks Apps can't be public, so a phone carrier can't reach them. A small stateless Rust relay outside Databricks (same AWS region) must receive carrier traffic and forward it to the app with a service-principal token. A public web widget needs the same relay. (07)
2. **Databricks has no built-in speech-to-text or text-to-speech, and no streaming audio.** We self-host Parakeet (STT) and Kokoro (TTS) on GPUs kept warm 24/7, about $1k/month each. Requests are per utterance and per sentence, which puts the estimated phone latency at **1.1–1.3 s P50**, above the 1 s goal. (01, 07)
3. **There's a fixed monthly cost floor.** Warm GPUs plus Apps cost about $2.5–5.3k/month before any calls, plus about $0.014/min. A hosted-vendor stack costs about $0.047/min with no floor. Databricks becomes cheaper above roughly 75–160k call-minutes a month. Dev and staging can scale to zero. (07)
4. **Open-weight TTS lags hosted TTS.** The best commercially usable open TTS ranks #24 on Artificial Analysis, and Kokoro ranks #62, versus #1 for ElevenLabs. Callers will notice. (01)
5. **Latency is a legal requirement.** US outbound AI telemarketing must open within 2 s of "hello", with identity, purpose and an opt-out. (04)
6. **Compliance is the product wedge.** None of the six self-serve competitors checks Do-Not-Call lists, enforces calling hours in the recipient's time zone, or keeps a consent ledger. (04)

## Recommended stack

| Area | Recommendation | Report |
|---|---|---|
| Hosting | Databricks Apps; the Rust binary is shipped as a Python wheel from a Unity Catalog Volume (the per-file limit is 10 MB) | 07 |
| Edge | Small Rust relay on AWS: verifies carrier signatures and forwards media over WebSocket | 07 |
| Database | Lakebase (Postgres, GA) with a branch per PR; local Postgres in Docker on the same major version | 02, 07 |
| LLM | Start on Databricks-hosted **Claude Haiku 5.5** (streaming, strong tool use, no GPU floor). Bake it off against self-hosted **Qwen3.6-35B-A3B** in spike S5. Avoid GPT-OSS for voice, because its thinking can't be turned off. | 01, 07, 09 |
| STT | Parakeet TDT 0.6B v2 per utterance; Qwen3-ASR for multilingual | 01, 07 |
| TTS | Kokoro-82M for MVP; quality bake-off before GA (Qwen3-TTS, Ming-3.0, Chatterbox) | 01 |
| Turn detection | Silero VAD + Smart Turn v3, inside the Rust process | 01 |
| Per-turn decisions | Jev System One (`openjev-qwen35-4b`): branch routing, intent, opt-out, escalation, run in parallel with the reply | 08 |
| Test scoring | Jev `score` questions against a rubric; publish is gated on the results | 08 |
| Simulated caller | Nemotron 3.5 Bolt E4B (also a light local-dev model) | 09 |
| Telephony | Telnyx primary, Twilio as second adapter | 01 |
| Backend | axum 0.8, tokio ≥1.54.1, sqlx 0.9, utoipa 6 (code-first OpenAPI → `contracts/openapi.json`), apalis-postgres jobs, tracing + OpenTelemetry | 02 |
| Layer enforcement | Crates per layer + `xtask check-layers` (cargo metadata) + cargo-deny `wrappers` | 02 |
| Frontend | React 19 + Vite SPA, TypeScript 7, shadcn/Base UI + Tailwind, TanStack Query/Router, XState, orval, React Flow | 03 |
| FE enforcement | dependency-cruiser, Oxlint (type-aware) + Oxfmt, Vitest, Playwright, Storybook | 03 |
| Gates | Claude Code hooks: format/lint after each edit, async Graphify update, Stop hook runs `just verify` + relevant reviewers. Then lefthook git hooks, then CI. | 05, 06 |
| CI | One workflow; a single always-run "gate" job is the only required check; actions pinned by SHA | 06 |
| Contracts | Signed tags `contract/<feature>/vN` plus a manifest; CI fails on any change to tagged files | 06 |
| Graph | Graphify (`graphifyy` on PyPI, pinned); local parsing, no LLM; `graphify-out/` gitignored | 06 |

## Local development (macOS)

- The Rust server runs natively.
- Postgres runs in Docker.
- Models run natively, because Docker on Mac can't use the GPU:
  - `llama-server` for the LLM
  - `mlx-audio` for STT and TTS
  - llama.cpp for the Jev-compatible decision API, using the Apache-2.0 Kev-4B model
- Adapters differ from production only by base URL and auth. (07, 08, 09)

## Spikes before committing (about 2 weeks)

| Spike | Proves |
|---|---|
| S1 | The Rust binary runs on Databricks Apps (via wheel) |
| S2 | WebSockets stay up 30+ minutes through the Apps proxy |
| S3 | STT latency and accuracy on 8 kHz phone audio |
| S4 | TTS time to first audio |
| S5 | LLM time to first token over 24 h: Haiku 5.5 vs self-hosted Qwen3.6 |
| S6 | One full phone call end to end through the relay |
| S7 | Lakebase token refresh in sqlx; branch per CI run |
| S8 | Jev decision latency on Databricks (not yet measured) |

## Conflicts resolved between reports

- **LLM:** 01 and 09 pick self-hosted Qwen3.6; 07 picks hosted Haiku 5.5 or GPT-OSS. Resolution: start hosted (no GPU floor, better tool use), bake off in S5, and keep both behind the same trait. GPT-OSS is dropped for voice (01).
- **Postgres version:** 02 says 18; Lakebase supports 16–18. Use 18 if the Lakebase region offers it, and pin local dev to match.
- **UX laws:** the brief lists 18, not 19. "Minimize target distance" is part of Fitts's law.

## Decisions (2026-10-30)

| # | Decision | Outcome |
|---|---|---|
| D1 | Phone calls and the public relay | **Browser only for now.** No telephony and no relay in the first release. Operators test agents in the browser through Databricks login. The telephony port stays in the design, and the relay comes with phone support later. |
| D2 | GitHub account and visibility | **Public repo in a free organization.** All gates, $0. A license still has to be chosen. |
| D3 | Speech and model hosting | **Local models in local development; Databricks models when deployed. No outside speech or LLM vendors.** We accept the latency and voice quality the spikes measure. |
| D4 | Launch scope | **Configurable.** Use case (support, service, telemarketing, survey, …) and jurisdiction are per-agent settings that select a compliance rule pack, not hard-coded product choices. |
| D5 | Backend and frontend stacks | Recommended stacks above. To be confirmed in the plan review. |
| D6 | Separate GitHub identity for agent PRs | Recommended; to be confirmed at repo setup. |

Consequences:
- **Dropped from MVP:** the edge relay and the telephony adapters. Spike S6 becomes "one full browser call end to end".
- **Better accuracy:** browser audio is 16 kHz or more, which improves STT accuracy compared with 8 kHz phone audio.
- **Compliance becomes data, not code paths:** rule packs (consent, disclosures, calling hours, opt-out) are versioned configuration selected per agent. The domain model includes them from day one, even though outbound dialing ships later.
