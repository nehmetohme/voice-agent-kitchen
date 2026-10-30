#!/usr/bin/env bash
# Runs from the canary worktree's root. `cargo add` instead of a diff: it edits whatever
# vak-domain's manifest and Cargo.lock look like today, so the violation never goes stale.
set -euo pipefail
cd backend
cargo add --quiet --package vak-domain --path crates/services
