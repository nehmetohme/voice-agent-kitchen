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
| Local dev | Local models (llama.cpp, MLX) and Postgres in Docker |

## Documentation

- [Project brief](PROMPT.md)
- [Research summary](docs/research/README.md)
- [Implementation plan](docs/plan/README.md)

## Development

Every change follows test-driven development:
- Contract tests are written first and pinned by signed tags.
- Each task gets its own git worktree.
- Changes merge only when CI is green.

See [docs/plan/quality.md](docs/plan/quality.md).

## License

[Apache-2.0](LICENSE)
