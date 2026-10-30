# Contract recipes, imported by the root justfile (recipes run from the repo root):
# OpenAPI and generated-client drift, breaking API changes, and contract files pinned by
# signed contract/<feature>/v<N> tags. See contracts/CLAUDE.md.

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

# OWNER ONLY: sign contract/<feature>/v<version> at HEAD and push it. Agents never run this.
[positional-arguments]
contract-approve feature version:
    #!/usr/bin/env bash
    set -euo pipefail
    feature="$1" version="$2"
    tag="contract/$feature/v$version"
    echo "WARNING: OWNER ONLY. This signs $tag with your key and pushes it: the signature" >&2
    echo "         is your approval of the contract tests. Agents and CI never run this." >&2
    die() { echo "contract-approve: $*" >&2; exit 1; }
    [[ "$feature" =~ ^[a-z0-9][a-z0-9-]*$ ]] || die "feature must be lowercase letters, digits and '-'"
    [[ "$version" =~ ^[1-9][0-9]*$ ]] || die "version must be a positive whole number"
    [[ -t 0 ]] || die "interactive only: run it yourself in a terminal"
    [[ -z "$(git status --porcelain)" ]] || die "the working tree is not clean: commit or stash first"
    pinned="$(git show HEAD:contracts/contracts.toml 2>/dev/null | taplo get "features.$feature.version" 2>/dev/null || true)"
    [[ "$pinned" == "$version" ]] ||
        die "HEAD's contracts/contracts.toml has $feature at version '${pinned:-none}', not $version"
    if git rev-parse --quiet --verify "refs/tags/$tag" >/dev/null; then
        die "$tag already exists; signed versions never move: bump the version instead"
    fi
    echo "About to sign $tag at $(git log -1 --format='%h %s' HEAD)" >&2
    read -r -p "Type the tag name to confirm: " answer
    [[ "$answer" == "$tag" ]] || die "not confirmed; nothing signed"
    git tag -s "$tag" -m "$feature v$version" HEAD
    # Catch a wrong user.signingkey before anything leaves this machine.
    signers="$(mktemp)"
    trap 'rm -f "$signers"' EXIT
    git show HEAD:.github/contract-signers > "$signers"
    if ! git -c gpg.ssh.allowedSignersFile="$signers" verify-tag "$tag"; then
        git tag -d "$tag" >/dev/null
        die "the signature doesn't match a key in .github/contract-signers (check user.signingkey); tag removed"
    fi
    git push origin "$tag"
    echo "contract-approve: pushed $tag" >&2

# CI: run the BASE ref's copy of contract-check (from a temporary worktree) against HEAD
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
    echo "== contract-check: contract files vs. signed contract tags (base {{ contracts_base }}) =="
    if ! (cd backend && cargo xtask contract-check --base '{{ contracts_base }}' --head HEAD); then
        echo "contracts: every other contract gate passed; contract-check failed (above)." >&2
        echo "contracts: untagged files wait for the owner's signature: see \`just contract-status\`." >&2
        exit 1
    fi
