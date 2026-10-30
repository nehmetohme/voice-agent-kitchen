#!/usr/bin/env bash
# Runs from the canary worktree's root. `cargo add` instead of a diff: it edits whatever
# vak-api's manifest and Cargo.lock look like today, so the violation never goes stale.
#
# The API reaching past services into persistence: an edge layers.toml forbids that can never
# become a cycle, since vak-repositories-pg may never depend on vak-api. (vak-domain ->
# vak-services can: once services depends on the domain, as layers.toml allows, cargo rejects
# the cycle before check-layers runs.) The domain's own rule is the layer-tokio canary.
set -euo pipefail
cd backend
cargo add --quiet --package vak-api --path crates/repositories-pg
