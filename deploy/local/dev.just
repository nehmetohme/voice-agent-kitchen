# Local dev stack recipes (deploy/local/), imported by the root justfile; they run from the repo
# root. Settings come from .env through deploy/local/env.sh. See deploy/CLAUDE.md.

# process-compose control socket: `dev-down` finds the running stack through it.
dev_socket := env('VAK_PC_SOCKET', env('TMPDIR', '/tmp') / 'vak-dev.sock')

# Start the whole local stack under process-compose (extra args go to `process-compose up`, e.g. --detached)
[positional-arguments]
dev *args: dev-setup
    #!/usr/bin/env bash
    set -euo pipefail
    . deploy/local/env.sh
    deploy/local/models.sh --check
    # Build once up front, so the two `cargo run` processes don't queue on the build lock.
    (cd backend && cargo build --quiet --locked -p vak-server -p vak-model-shim)
    exec process-compose --use-uds --unix-socket '{{ dev_socket }}' --ordered-shutdown \
        up --disable-dotenv --config deploy/local/process-compose.yml "$@"

# Start only Postgres, vak-server and the frontend: no local models (extra args go to `process-compose up`)
[positional-arguments]
dev-light *args:
    #!/usr/bin/env bash
    set -euo pipefail
    . deploy/local/env.sh
    (cd backend && cargo build --quiet --locked -p vak-server)
    exec process-compose --use-uds --unix-socket '{{ dev_socket }}' --ordered-shutdown \
        up --disable-dotenv --config deploy/local/process-compose.yml "$@" vak-server frontend

# Stop the stack started by `just dev` (or `dev-light`) and the Postgres container; data is kept
dev-down:
    #!/usr/bin/env bash
    set -uo pipefail
    . deploy/local/env.sh
    if [ -S '{{ dev_socket }}' ] &&
        process-compose --use-uds --unix-socket '{{ dev_socket }}' down >/dev/null 2>&1; then
        echo "dev-down: stopped the process-compose stack"
    else
        echo "dev-down: no process-compose stack running"
    fi
    docker compose -f deploy/local/docker-compose.yml stop
    echo "dev-down: Postgres stopped (data kept in the vak-local_pgdata volume)"

# Start Postgres 18 in Docker and wait until it is healthy
db-up:
    #!/usr/bin/env bash
    set -euo pipefail
    . deploy/local/env.sh
    docker compose -f deploy/local/docker-compose.yml up --detach --wait db
    echo "db-up: Postgres 18 on 127.0.0.1:$VAK_PG_PORT (DATABASE_URL in .env)"

# Download the local models (~5.4 GiB) into VAK_MODELS_DIR, verified by SHA-256; idempotent
models-pull:
    #!/usr/bin/env bash
    set -euo pipefail
    . deploy/local/env.sh
    deploy/local/models.sh

# Sync the pinned mlx-audio environment (deploy/local/mlx-audio, uv) into its project-local .venv
dev-setup:
    deploy/local/mlx-audio.sh --sync

# Check every dependency of the local stack, the LLM box included, with fix hints
dev-doctor:
    deploy/local/doctor.sh

# Call every endpoint of the running stack end to end (first and warm) and print latencies
dev-smoke:
    deploy/local/smoke.sh

# Lint the local stack's scripts (part of `just repo-verify-fast`)
dev-lint:
    shellcheck -x deploy/local/*.sh
