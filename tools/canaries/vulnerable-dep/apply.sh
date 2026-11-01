#!/usr/bin/env bash
# Runs from the canary worktree's root. `cargo add` instead of a diff: a Cargo.lock hunk would
# go stale with every dependency bump. protobuf 2.28.0 has RUSTSEC-2024-0437 (a vulnerability,
# not just "unsound": cargo audit only warns on unsound advisories) and no default
# dependencies, so the advisory is the only finding. vak-audio has no external allowlist in
# layers.toml, so check-layers passes and the gate under test is the first to fail.
set -euo pipefail
cd backend
cargo add --quiet --package vak-audio protobuf@=2.28.0
