# Voice Agent Kitchen task runner. Run `just` to list recipes.
# Each area owns its own .just file; this root file only composes them.

set shell := ["bash", "-euo", "pipefail", "-c"]

import 'backend/backend.just'
import 'frontend/frontend.just'
import 'tools/repo.just'
import 'tools/ci.just'
import 'contracts/contracts.just'
import 'deploy/local/dev.just'

# List recipes
default:
    @just --list

# Contracts run last: when everything else is green, a contract file still waiting for the
# owner's signed tag is the only failure left (see contracts/CLAUDE.md).

# Every gate: must pass before a task is done (hooks and CI run this)
verify: backend-verify frontend-verify repo-verify ci-verify contracts-verify

# Quick subset for pre-commit and post-edit checks
verify-fast: backend-verify-fast frontend-verify-fast repo-verify-fast ci-verify-fast contracts-verify-fast
