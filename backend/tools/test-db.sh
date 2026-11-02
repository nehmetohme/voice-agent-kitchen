#!/usr/bin/env bash
# This worktree's throwaway Postgres for tests: `#[sqlx::test]` (through the nextest setup
# script in .config/nextest.toml) and `cargo sqlx prepare` (`just db-prepare`).
#
#   test-db.sh url       start the container if needed, print its DATABASE_URL
#   test-db.sh nextest   the same, written to $NEXTEST_ENV for the tests that need it
#   test-db.sh stop      remove the container
#
# Each worktree gets its own container and port, derived from a checksum of its path, so
# parallel worktrees never share a database: port 25200-25999 (VAK_TEST_DB_PORT overrides it).
# The container keeps its data in memory, trusts connections only from 127.0.0.1 and holds
# nothing but test data; `#[sqlx::test]` creates and drops one database per test inside it.
set -euo pipefail

image=${VAK_TEST_DB_IMAGE:-postgres:18.6-alpine}
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
checksum=$(printf '%s' "$root" | cksum | cut -d' ' -f1)
port=${VAK_TEST_DB_PORT:-$((25200 + checksum % 800))}
name="vak-test-db-$checksum"
url="postgres://postgres@127.0.0.1:$port/postgres"

die() {
  echo "test-db: $*" >&2
  exit 1
}

running() {
  [ "$(docker inspect --format '{{.State.Running}}' "$name" 2>/dev/null || true)" = true ]
}

start() {
  command -v docker >/dev/null || die "docker is not installed; the Postgres tests need it"
  docker info >/dev/null 2>&1 || die "docker is not running; start Docker and retry"
  if ! running; then
    docker rm --force "$name" >/dev/null 2>&1 || true
    docker run --detach --quiet --rm --name "$name" \
      --publish "127.0.0.1:$port:5432" \
      --env POSTGRES_HOST_AUTH_METHOD=trust \
      --tmpfs /var/lib/postgresql \
      "$image" \
      -c fsync=off -c synchronous_commit=off -c full_page_writes=off -c max_connections=300 \
      >/dev/null || die "could not start $image on port $port"
  fi
  # The image's init server listens on a socket only; TCP answers once the real server runs.
  for _ in $(seq 1 120); do
    if docker exec "$name" pg_isready --quiet --host 127.0.0.1 --username postgres; then
      return 0
    fi
    sleep 0.5
  done
  docker logs --tail 20 "$name" >&2 || true
  die "Postgres in $name did not become ready within 60 s"
}

case "${1:-url}" in
  url)
    start
    echo "$url"
    ;;
  nextest)
    [ -n "${NEXTEST_ENV:-}" ] || die "NEXTEST_ENV is not set; nextest runs this as a setup script"
    start
    echo "DATABASE_URL=$url" >>"$NEXTEST_ENV"
    ;;
  stop)
    docker rm --force "$name" >/dev/null 2>&1 || true
    ;;
  *)
    die "usage: test-db.sh [url|nextest|stop]"
    ;;
esac
