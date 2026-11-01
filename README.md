# Voice Agent Kitchen

A platform for creating, testing and publishing AI voice agents: customer support, customer service, booking, surveys, telemarketing and more.

**Flow:** Provision → Plan → Test → Publish.

**Status:** Milestone M0 (foundation) is in progress. There is no product code yet.

## Stack

| Part | Technology |
|---|---|
| Backend | Rust (axum, tokio, sqlx), layered Cargo workspace |
| Frontend | React + Vite + TypeScript |
| Production | Databricks Apps, Model Serving, Lakebase (Postgres) |
| Local dev | KAT-Coder LLM on a LAN GPU box; speech (MLX) and decisions (llama.cpp) on the Mac; Postgres in Docker |

## Documentation

- [Project brief](PROMPT.md)
- [Research summary](docs/research/README.md)
- [Implementation plan](docs/plan/README.md)

## Development

**Prerequisites:** an Apple Silicon Mac with Docker Desktop, Homebrew `just`, `uv`, `llama.cpp`, `process-compose`, `lefthook`, Rust (rustup) and `pnpm`; and the LAN GPU box serving the KAT-Coder LLM.

**One-time setup**, from the repo root:

```sh
just setup              # git hooks
cp .env.example .env    # then set VAK_LLM_BASE_URL (the GPU box) and VAK_PG_PASSWORD
just models-pull        # Parakeet, Kokoro and Kev-4B (5.4 GiB) into ~/.cache/vak/models
just dev-doctor         # checks every dependency, the LLM box included
```

**Every day:** `just dev` starts the whole local stack (Postgres, speech, decisions, model shim, server on :3000, frontend on :5173). `just dev-smoke` calls every endpoint and prints latencies, and `just dev-down` stops it all. Details: [deploy/CLAUDE.md](deploy/CLAUDE.md).

Every change follows test-driven development:
- Contract tests are written first and pinned by signed tags.
- Each task gets its own git worktree.
- Changes merge only when CI is green.

See [docs/plan/quality.md](docs/plan/quality.md).

## License

[Apache-2.0](LICENSE)
