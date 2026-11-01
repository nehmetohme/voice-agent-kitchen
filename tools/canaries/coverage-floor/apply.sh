#!/usr/bin/env bash
# Runs from the canary worktree's root. Adds a public vak-domain function that no test calls.
# A script instead of a diff: lib.rs grows with every milestone, so a diff would go stale.
#
# The function is sized from the crate: one uncovered line per five source lines (at least
# 40). Falling below the 95 % floor takes more than 5.3 % of the crate's lines uncovered, so
# this keeps a margin of almost 4x however large vak-domain grows.
set -euo pipefail
src=backend/crates/domain/src
lines=$(find "$src" -name '*.rs' -exec cat {} + | wc -l)
steps=$((lines / 5))
if [ "$steps" -lt 40 ]; then steps=40; fi

{
  echo '//! Canary (tools/canaries/coverage-floor): code that no test runs.'
  echo
  echo '/// A score that no test computes.'
  echo '#[must_use]'
  echo 'pub fn untested_score(seed: u64) -> u64 {'
  echo '    let mut score = seed;'
  i=0
  while [ "$i" -lt "$steps" ]; do
    echo "    score = score.wrapping_mul(31).wrapping_add($i);"
    i=$((i + 1))
  done
  echo '    score'
  echo '}'
} >"$src/canary_untested.rs"
printf '\npub mod canary_untested;\n' >>"$src/lib.rs"
