# deploy/: infrastructure as code

## Responsibility

| Path | What |
|---|---|
| `local/` | The local dev stack on macOS (Apple Silicon): Postgres 18 in Docker, the speech and decision models native, the LLM on a LAN GPU box |
| `databricks/` | The Databricks bundle, serving wrappers and wheel packaging (M7; not here yet) |

Production runs only on Databricks. Local development swaps in local servers behind the same wire contracts, so the two differ in configuration only: base URLs, model IDs and auth.

## The local stack (`local/`)

| Part | Local | Port | Production |
|---|---|---|---|
| LLM | KAT-Coder (`KAT-Coder-V2.5-Dev`) on a LAN GPU box, llama-server, OpenAI-compatible chat with streaming and tools. One slot. Voice turns send `chat_template_kwargs.enable_thinking=false` (`VAK_LLM_EXTRA_BODY`) | `VAK_LLM_BASE_URL` | FMAPI / Unity Gateway |
| STT, TTS | mlx-audio 0.6.3 (Parakeet TDT 0.6B v2, Kokoro-82M on MLX) behind `vak-model-shim` | 8000 → 8001 | Model Serving pyfuncs |
| Decisions | Homebrew `llama-server` with Kev-4B Q4_K_M, `/v1/systemone` | 8081 | Unity Gateway System One |
| Database | `postgres:18.6-trixie`, pinned by digest | 5433 | Lakebase |
| App | `vak-server migrate`, then `vak-server serve` (`cargo run`), Vite dev server | 3000, 5173 | Databricks App |

| File | Does |
|---|---|
| `local/dev.just` | the recipes below; imported by the root `justfile` |
| `local/env.sh` | loads the repo's `.env` and fills in defaults; sourced by every script and recipe |
| `local/process-compose.yml` | the processes, their readiness probes and start order |
| `local/docker-compose.yml` | Postgres 18 with a healthcheck and the `pgdata` volume (`vak-local_pgdata`, or `<COMPOSE_PROJECT_NAME>_pgdata`) |
| `local/models.sh` | downloads the pinned model files into `VAK_MODELS_DIR`, verified by size and SHA-256 |
| `local/mlx-audio/` | `pyproject.toml` + `uv.lock`: the pinned mlx-audio environment, synced into a project-local `.venv`. No code |
| `local/mlx-audio.sh`, `local/speech-warmup.sh` | start mlx-audio; load and warm both speech models before the shim starts |
| `local/doctor.sh`, `local/smoke.sh` | `dev-doctor` and `dev-smoke` |

**Models** live outside the repo in `${VAK_MODELS_DIR:-~/.cache/vak/models}`, so every worktree shares one copy (5.4 GiB). `models.sh` pins each file to a Hugging Face commit and a SHA-256; add a model there, never by hand. It pins a Kokoro voice file for every voice of the agent catalog (`VOICES` in `backend/crates/domain/src/catalog.rs`): mlx-audio runs offline and loads voices from the model folder only, so a missing one fails every call with that voice. A `vak-server` test fails when the two drift, and `just dev-doctor` names a voice that isn't downloaded. mlx-audio loads a new voice file on its next call, with no restart. Licences: Parakeet CC-BY-4.0, Kokoro and Kev-4B Apache-2.0. Never fetch `openjev/openjev` (CC-BY-NC); the script refuses it. Kokoro's G2P in the mlx-audio environment pulls in copyleft packages: `phonemizer-fork` and espeak-ng (GPL-3.0), and `num2words` (LGPL). They are fine on a developer's Mac and are exempted by package in `.github/dependency-review.yml`, but they must never ship. The production Kokoro pyfunc (M7) needs its own licence review.

**Configuration** is the gitignored `.env` at the repo root (`cp .env.example .env`). `.env.example` lists every variable with placeholders; the real LAN address and passwords stay in `.env`. `.worktreeinclude` copies `.env` into worktrees that Claude Code creates; for a `just task` worktree, copy it from the main checkout (`just dev-doctor` prints the command).

**Identity:** in production the Databricks Apps proxy adds `X-Forwarded-User` and `X-Forwarded-Email`. Locally nothing does, so `env.sh` sets `VAK_DEV_OPERATOR_EMAIL=dev@localhost` and API requests without the headers act as that operator. `vak-config` refuses the variable outside the `local` profile. Set it empty in `.env` to require the headers.

**Tool hosts:** `VAK_TOOL_HOSTS` is the allow-list of hosts purposes' custom HTTP tools may call (comma-separated; `*.example.com` for subdomains). Unset means none: a purpose with an HTTP tool can't save a version until its host is listed. Nothing calls those hosts before M4b.

## Conventions

- **No secrets in files.** `.env` is gitignored; `.env.example` holds placeholders only.
- **Pin everything:** container images by digest, model files by commit and SHA-256, Python packages by `uv.lock`. CI's dependency review scans `uv.lock` too: `mlx-audio/pyproject.toml` overrides setuptools to a patched version and swaps webrtcvad for `webrtcvad-wheels` (same module, no `pkg_resources`).
- **Native models, Docker only for Postgres:** containers on macOS get no Metal GPU.
- **Bind to 127.0.0.1.** Nothing local listens on the LAN.
- **Scripts:** `bash` with `set -euo pipefail` (doctor and smoke collect failures with `-uo`). Every network call has a timeout (`curl -m`). They must be shellcheck-clean: `just repo-hooks-test` (in `just verify` and CI) checks `local/*.sh` with `--source-path=SCRIPTDIR`, so write `# shellcheck source=env.sh`.
- **Keep it in step with the bundle (M7):** a model or contract change lands in both, or the PR says why not.
- **Known gap:** espeak-ng (Kokoro's out-of-vocabulary fallback) keeps its data path in a 160-byte buffer, and a worktree's `.venv` path is longer, so `mlx-audio.sh` passes a short symlink in `TMPDIR` as `ESPEAK_DATA_PATH`.

## Commands

| Recipe | Does |
|---|---|
| `just dev [args]` | the whole stack under process-compose (TUI; `--detached` for background). Checks the models, syncs mlx-audio, builds the Rust binaries first |
| `just dev-light [args]` | Postgres, `vak-server` and the frontend only, no local models. It clears the speech and decision URLs, so `vak-server` reports voice sessions as off and refuses them |
| `just dev-down` | stops the process-compose stack and the Postgres container; data is kept, in the volume it names |
| `just db-up` | Postgres only, waits until healthy |
| `just models-pull` | downloads missing models; idempotent |
| `just dev-setup` | syncs the mlx-audio environment |
| `just dev-doctor` | checks tools, Docker, `.env`, the LLM box, models (each missing catalog voice by name), `/v1/systemone` support and ports, with fix hints |
| `just dev-smoke` | calls every endpoint of the running stack twice (first, warm): LLM with thinking off, STT on `backend/fixtures/audio/utterance_book_table_16k.wav`, TTS, a 4-question decision, Postgres `select 1`, `vak-server`, Vite, and `GET /api/v1/agents` through Vite's proxy (proves the migrations ran and the dev operator is set) |
| `just adapters-live [args]` | runs the four model adapters' ignored live contract tests against the running stack, one test at a time (the LLM has one slot), with `local/env.sh` loaded. Needs `just dev` and the LAN LLM in `.env`. Defined in `backend/backend.just`; `-p <crate>` runs one adapter |

## Testing

- `vak-model-shim` has its own contract tests (`backend/apps/model-shim`), against wiremock.
- The stack itself is tested by `just dev-smoke` against the real models. Run it after changing anything here, and put its latencies in the PR.
- `just adapters-live` checks that the Rust adapters still pass their port's conformance suite against these servers. Run it after changing a model, an endpoint or a model variable (`VAK_LLM_*`, `VAK_STT_*`, `VAK_TTS_*`, `VAK_DECISION_*`).

## Example

```sh
just setup && cp .env.example .env   # then set VAK_LLM_BASE_URL and VAK_PG_PASSWORD
just models-pull && just dev-doctor
just dev --detached && just dev-smoke && just dev-down
```
