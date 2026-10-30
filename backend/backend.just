# Backend recipes (Rust workspace). Imported by the root justfile; every recipe runs in
# backend/. See backend/CLAUDE.md.

# Files left out of the coverage report, on top of cargo-llvm-cov's defaults (tests/,
# examples/, benches/): binaries, automation and dev-only crates.
backend_coverage_ignore := '/backend/(apps|xtask|crates/testkit|crates/contract-tests)/'

# All backend gates (the backend part of `just verify`)
[working-directory: 'backend']
backend-verify: backend-verify-fast backend-coverage
    cargo test --workspace --all-features --locked --doc
    cargo deny check
    cargo audit
    cargo shear

# Fast backend gates: format, lint, layer rules (the backend part of `just verify-fast`)
[working-directory: 'backend']
backend-verify-fast:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo xtask check-layers

# Run the backend tests: nextest, then doctests. Extra args go to nextest, e.g. `-p vak-api`
[working-directory: 'backend']
backend-test *args:
    cargo nextest run --all-features {{ args }}
    cargo test --workspace --all-features --doc

# Regenerate the sqlx offline query cache (crates/repositories-pg/.sqlx) against a freshly
# migrated database in this worktree's test container. Commit the result.
[working-directory: 'backend']
db-prepare:
    #!/usr/bin/env bash
    set -euo pipefail
    server="$(tools/test-db.sh url)"
    url="${server%/postgres}/vak_prepare"
    sqlx database drop -y --database-url "$url" >/dev/null
    sqlx database create --database-url "$url"
    sqlx migrate run --source crates/repositories-pg/migrations --database-url "$url"
    cd crates/repositories-pg
    DATABASE_URL="$url" SQLX_OFFLINE=false cargo sqlx prepare -- --all-targets --all-features
    echo "db-prepare: regenerated crates/repositories-pg/.sqlx; commit it"

# Stop and remove this worktree's test Postgres container (the tests restart it on demand)
[working-directory: 'backend']
db-test-stop:
    tools/test-db.sh stop

# Format Rust code and backend TOML files
[working-directory: 'backend']
backend-fmt:
    cargo fmt --all
    taplo fmt

# Tests under coverage instrumentation (verify's nextest run), then the coverage.toml floors
[working-directory: 'backend']
backend-coverage:
    cargo llvm-cov clean --workspace
    cargo llvm-cov nextest --workspace --all-features --locked --no-report
    mkdir -p target/llvm-cov
    cargo llvm-cov report --json --summary-only --ignore-filename-regex '{{ backend_coverage_ignore }}' --output-path target/llvm-cov/summary.json
    cargo xtask coverage-gate target/llvm-cov/summary.json
