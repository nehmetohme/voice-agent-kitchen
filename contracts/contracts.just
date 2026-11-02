# Contract recipes, imported by the root justfile (recipes run from the repo root):
# OpenAPI and generated-client drift, breaking API changes, and contract files pinned by
# contract/<feature>/v<N> tags. See contracts/CLAUDE.md.
#
# Signatures are off for now: contract-pin makes unsigned tags, and contract-check runs
# without --require-signatures. To require them again, add that flag to every contract-check
# call below and bring back a signing recipe (contracts/CLAUDE.md, "Signatures").

# Trusted base for `oasdiff breaking` and contract-check. CI sets it to the PR's base SHA.
# Locally, `git fetch origin main` first so the base is current.
contracts_base := env('VAK_CONTRACTS_BASE', 'origin/main')

# Every contract gate; contract-check runs last and reports on its own (`just verify`)
contracts-verify: _contracts-openapi-drift _contracts-client-drift _contracts-breaking _contracts-check

# Contract gates without regenerating the frontend client (`just verify-fast`)
contracts-verify-fast: _contracts-openapi-drift _contracts-breaking _contracts-check

# Regenerate contracts/openapi.json from the Rust API, then the frontend client from it
openapi: _frontend-deps
    cd backend && cargo run -p vak-server --quiet --locked -- openapi --output ../contracts/openapi.json
    cd frontend && pnpm run --silent api:generate
    @echo "contracts: regenerated contracts/openapi.json and frontend/src/shared/api/generated/; commit both"

# List contract features with their version and tag state, and any untagged contract files
contract-status base=contracts_base:
    cd backend && cargo xtask contract-status --base '{{ base }}' --head HEAD

# COORDINATING SESSION ONLY: create the unsigned annotated tag contract/<feature>/v<version> at HEAD (never pushes, moves or deletes a tag; push it with contract-push)
[positional-arguments]
contract-pin feature version:
    #!/usr/bin/env bash
    set -euo pipefail
    feature="$1" version="$2"
    die() { echo "contract-pin: $*" >&2; exit 1; }
    [[ "$feature" =~ ^[a-z0-9][a-z0-9-]*$ ]] || die "feature must be lowercase letters, digits and '-'"
    [[ "$version" =~ ^[1-9][0-9]*$ ]] || die "version must be a positive whole number"
    tag="contract/$feature/v$version"
    command -v taplo >/dev/null || die "taplo is required: it reads HEAD's contracts/contracts.toml"
    [[ -z "$(git status --porcelain)" ]] ||
        die "the working tree is not clean: commit first, so the tag pins exactly what HEAD holds"
    manifest="$(git show HEAD:contracts/contracts.toml 2>/dev/null)" ||
        die "HEAD has no contracts/contracts.toml"
    pinned="$(taplo get "features.$feature.version" <<<"$manifest" 2>/dev/null || true)"
    [[ "$pinned" == "$version" ]] ||
        die "HEAD's contracts/contracts.toml has $feature at version '${pinned:-none}', not $version"
    if git rev-parse --quiet --verify "refs/tags/$tag" >/dev/null; then
        die "$tag already exists locally; a contract tag never moves: bump the version instead"
    fi
    remote="$(git ls-remote --refs origin "refs/tags/$tag")" ||
        die "can't list origin's tags, so can't tell whether $tag already exists there"
    if [[ -n "$(awk -v ref="refs/tags/$tag" '$2 == ref' <<<"$remote")" ]]; then
        die "$tag already exists on origin; a contract tag never moves: bump the version instead"
    fi
    # Unsigned on purpose, whatever the git config says: signatures are off (contracts/CLAUDE.md).
    git -c tag.gpgSign=false -c tag.forceSignAnnotated=false \
        tag --annotate --message "$feature v$version" "$tag" HEAD
    echo "contract-pin: created $tag at $(git log -1 --format='%h %s' HEAD) (not pushed)" >&2
    echo "contract-pin: push it with \`just contract-push $feature $version\`" >&2

# COORDINATING SESSION ONLY: push exactly refs/tags/contract/<feature>/v<version> to origin, through the pre-push hook (never forces, moves or deletes a tag)
[positional-arguments]
contract-push feature version:
    #!/usr/bin/env bash
    set -euo pipefail
    feature="$1" version="$2"
    die() { echo "contract-push: $*" >&2; exit 1; }
    [[ "$feature" =~ ^[a-z0-9][a-z0-9-]*$ ]] || die "feature must be lowercase letters, digits and '-'"
    [[ "$version" =~ ^[1-9][0-9]*$ ]] || die "version must be a positive whole number"
    tag="contract/$feature/v$version"
    ref="refs/tags/$tag"
    local_id="$(git rev-parse --quiet --verify "$ref")" ||
        die "$tag doesn't exist locally: create it with \`just contract-pin $feature $version\`"
    remote="$(git ls-remote --refs origin "$ref")" ||
        die "can't list origin's tags, so can't tell whether $tag already exists there"
    remote_id="$(awk -v ref="$ref" '$2 == ref { print $1 }' <<<"$remote")"
    if [[ "$remote_id" == "$local_id" ]]; then
        echo "contract-push: origin already has $tag at the same tag object; nothing to push" >&2
        exit 0
    fi
    [[ -z "$remote_id" ]] ||
        die "origin has $tag at $remote_id, not the local $local_id; a contract tag never moves: bump the version instead"
    # One explicit refspec and --no-follow-tags: nothing else leaves. The pre-push hook runs.
    git push --no-follow-tags origin "$ref:$ref"
    echo "contract-push: pushed $tag" >&2

# CI: run the BASE ref's copy of contract-check (from a temporary worktree) against HEAD, strict
# (no --allow-pending): a contract still awaiting its tag blocks the merge
[positional-arguments]
contracts-check-ci base head:
    #!/usr/bin/env bash
    set -euo pipefail
    base="$1" head="$2"
    base_sha="$(git rev-parse --verify --quiet "$base^{commit}")" ||
        { echo "contracts-check-ci: unknown base $base (fetch it first)" >&2; exit 2; }
    head_sha="$(git rev-parse --verify --quiet "$head^{commit}")" ||
        { echo "contracts-check-ci: unknown head $head" >&2; exit 2; }
    repo="$(git rev-parse --show-toplevel)"
    tmp="$(mktemp -d)"
    cleanup() {
        git worktree remove --force "$tmp/base" >/dev/null 2>&1 || true
        rm -rf "$tmp"
    }
    trap cleanup EXIT
    # No hooks: the worktree is only a build input.
    git -c core.hooksPath=/dev/null worktree add --quiet --detach "$tmp/base" "$base_sha"
    target="$repo/backend/target/contracts-base"
    checker=""
    if [[ -f "$tmp/base/backend/xtask/Cargo.toml" ]]; then
        # A failed build of a trusted base is an error, never a reason to trust HEAD's copy.
        (cd "$tmp/base/backend" && CARGO_TARGET_DIR="$target" cargo build --quiet --locked -p vak-xtask --bin xtask)
        if "$target/debug/xtask" contract-check --help >/dev/null 2>&1; then
            checker="$target/debug/xtask"
        fi
    fi
    cd "$repo"
    if [[ -n "$checker" ]]; then
        echo "contracts-check-ci: running the base's contract-check ($base_sha)" >&2
        "$checker" contract-check --base "$base_sha" --head "$head_sha"
    else
        echo "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!" >&2
        echo "!! BOOTSTRAP: base $base_sha has no \`xtask contract-check\`." >&2
        echo "!! Running HEAD's copy instead. A pull request can change that copy, so only the" >&2
        echo "!! owner's review protects this run. Once contract-check is on the base branch," >&2
        echo "!! CI runs the base's copy." >&2
        echo "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!" >&2
        (cd backend && cargo xtask contract-check --base "$base_sha" --head "$head_sha")
    fi

[private]
_contracts-openapi-drift:
    #!/usr/bin/env bash
    set -euo pipefail
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    (cd backend && cargo run -p vak-server --quiet --locked -- openapi --output "$tmp/openapi.json")
    if ! diff -u contracts/openapi.json "$tmp/openapi.json"; then
        echo "contracts: contracts/openapi.json differs from the Rust API: run \`just openapi\` and commit" >&2
        exit 1
    fi
    echo "contracts: openapi.json matches the code"

[private]
_contracts-client-drift: _frontend-deps
    #!/usr/bin/env bash
    set -euo pipefail
    generated=frontend/src/shared/api/generated
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    mkdir -p "$tmp/before" "$generated"
    cp -R "$generated/." "$tmp/before/"
    (cd frontend && pnpm run --silent api:generate >/dev/null)
    if ! diff -ru "$tmp/before" "$generated"; then
        # Leave the tree as it was: verify reports, `just openapi` fixes.
        rm -rf "$generated" && mkdir -p "$generated" && cp -R "$tmp/before/." "$generated/"
        echo "contracts: $generated differs from contracts/openapi.json: run \`just openapi\` and commit" >&2
        exit 1
    fi
    echo "contracts: the generated client matches contracts/openapi.json"

[private]
_contracts-breaking:
    #!/usr/bin/env bash
    set -euo pipefail
    base='{{ contracts_base }}'
    git rev-parse --verify --quiet "$base^{commit}" >/dev/null ||
        { echo "contracts: unknown base $base: run \`git fetch origin main\`" >&2; exit 1; }
    if ! git cat-file -e "$base:contracts/openapi.json" 2>/dev/null; then
        echo "contracts: notice: $base has no contracts/openapi.json yet; skipping oasdiff breaking"
        exit 0
    fi
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    git show "$base:contracts/openapi.json" > "$tmp/base.json"
    oasdiff breaking "$tmp/base.json" contracts/openapi.json --fail-on ERR --allow-external-refs=false
    echo "contracts: no breaking API changes against $base"

[private]
_contracts-check:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "== contract-check (local: findings awaiting a contract tag warn; CI is strict) =="
    # --allow-pending: an agent must be able to commit and push contract tests before they are
    # pinned, so findings that only await the contract tag warn here. Violations of pinned
    # contracts still fail. CI (`contracts-check-ci`) runs strict and blocks the merge.
    if ! (cd backend && cargo xtask contract-check --allow-pending --base '{{ contracts_base }}' --head HEAD); then
        echo "contracts: every other contract gate passed; contract-check found a violation of a pinned contract (above)." >&2
        exit 1
    fi
