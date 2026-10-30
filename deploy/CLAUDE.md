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
| App | `vak-server` (`cargo run`), Vite dev server | 3000, 5173 | Databricks App |

| File | Does |
|---|---|
| `local/dev.just` | the recipes below; imported by the root `justfile` |
| `local/env.sh` | loads the repo's `.env` and fills in defaults; sourced by every script and recipe |
| `local/process-compose.yml` | the processes, their readiness probes and start order |
| `local/docker-compose.yml` | Postgres 18 with a healthcheck and the `vak-local_pgdata` volume |
| `local/models.sh` | downloads the pinned model files into `VAK_MODELS_DIR`, verified by size and SHA-256 |
| `local/mlx-audio/` | `pyproject.toml` + `uv.lock`: the pinned mlx-audio environment, synced into a project-local `.venv`. No code |
| `local/mlx-audio.sh`, `local/speech-warmup.sh` | start mlx-audio; load and warm both speech models before the shim starts |
| `local/doctor.sh`, `local/smoke.sh` | `dev-doctor` and `dev-smoke` |

**Models** live outside the repo in `${VAK_MODELS_DIR:-~/.cache/vak/models}`, so every worktree shares one copy (5.4 GiB). `models.sh` pins each file to a Hugging Face commit and a SHA-256; add a model there, never by hand. Licences: Parakeet CC-BY-4.0, Kokoro and Kev-4B Apache-2.0. Never fetch `openjev/openjev` (CC-BY-NC); the script refuses it.

**Configuration** is the gitignored `.env` at the repo root (`cp .env.example .env`). `.env.example` lists every variable with placeholders; the real LAN address and passwords stay in `.env`. `.worktreeinclude` copies `.env` into worktrees that Claude Code creates; for a `just task` worktree, copy it from the main checkout (`just dev-doctor` prints the command).

## Conventions

- **No secrets in files.** `.env` is gitignored; `.env.example` holds placeholders only.
- **Pin everything:** container images by digest, model files by commit and SHA-256, Python packages by `uv.lock`.
- **Native models, Docker only for Postgres:** containers on macOS get no Metal GPU.
- **Bind to 127.0.0.1.** Nothing local listens on the LAN.
- **Scripts:** `bash` with `set -euo pipefail` (doctor and smoke collect failures with `-uo`). Every network call has a timeout (`curl -m`). They must be shellcheck-clean: `just repo-hooks-test` (in `just verify` and CI) checks `local/*.sh` with `--source-path=SCRIPTDIR`, so write `# shellcheck source=env.sh`.
- **Keep it in step with the bundle (M7):** a model or contract change lands in both, or the PR says why not.
- **Known gap:** espeak-ng (Kokoro's out-of-vocabulary fallback) keeps its data path in a 160-byte buffer, and a worktree's `.venv` path is longer, so `mlx-audio.sh` passes a short symlink in `TMPDIR` as `ESPEAK_DATA_PATH`.

## Commands

| Recipe | Does |
|---|---|
| `just dev [args]` | the whole stack under process-compose (TUI; `--detached` for background). Checks the models, syncs mlx-audio, builds the Rust binaries first |
| `just dev-light [args]` | Postgres, `vak-server` and the frontend only, no local models |
| `just dev-down` | stops the process-compose stack and the Postgres container; data is kept |
| `just db-up` | Postgres only, waits until healthy |
| `just models-pull` | downloads missing models; idempotent |
| `just dev-setup` | syncs the mlx-audio environment |
| `just dev-doctor` | checks tools, Docker, `.env`, the LLM box, models, `/v1/systemone` support and ports, with fix hints |
| `just dev-smoke` | calls every endpoint of the running stack twice (first, warm): LLM with thinking off, STT on `backend/fixtures/audio/utterance_book_table_16k.wav`, TTS, a 4-question decision, Postgres `select 1`, `vak-server` and Vite |
## Testing

- `vak-model-shim` has its own contract tests (`backend/apps/model-shim`), against wiremock.
- The stack itself is tested by `just dev-smoke` against the real models. Run it after changing anything here, and put its latencies in the PR.

## Example

```sh
just setup && cp .env.example .env   # then set VAK_LLM_BASE_URL and VAK_PG_PASSWORD
just models-pull && just dev-doctor
just dev --detached && just dev-smoke && just dev-down
```
