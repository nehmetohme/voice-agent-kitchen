# Repo-wide recipes: secrets, TOML, workflows, contracts, worktrees.

# Repo-wide gates
repo-verify: repo-verify-fast repo-hooks-test

# Fast repo-wide gates
repo-verify-fast: repo-secrets
    taplo fmt --check

# Secret scan of tracked and new files only; gitignored build output (dist, target, ...) is skipped
repo-secrets:
    #!/usr/bin/env bash
    set -euo pipefail
    # gitleaks dir has no .gitignore support, so scan a snapshot of what git would see.
    snapshot=$(mktemp -d "${TMPDIR:-/tmp}/vak-secrets.XXXXXX")
    trap 'rm -rf "$snapshot"' EXIT
    git ls-files -z --cached --others --exclude-standard --deduplicate \
      | while IFS= read -r -d '' file; do
          if [ -f "$file" ] && [ ! -L "$file" ]; then printf '%s\0' "$file"; fi
        done \
      | tar --null -T - -cf - | tar -xf - -C "$snapshot"
    cd "$snapshot"
    gitleaks dir . --no-banner --redact

# Claude Code hook tests (claude and just are stubbed: no API calls, no network), plus shellcheck
repo-hooks-test:
    tools/tests/hooks/run.sh
    shellcheck -x --source-path=SCRIPTDIR .claude/hooks/*.sh .claude/hooks/lib/common.sh tools/tests/hooks/*.sh tools/tests/hooks/stubs/*

# Create a task worktree at .claude/worktrees/<name> on a new branch task/<name> from origin/main
task $name:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ ! "$name" =~ ^[a-z0-9][a-z0-9._-]*$ ]]; then
      echo "task: the name must be lowercase letters, digits, '.', '_' or '-'" >&2
      exit 1
    fi
    root=$(cd "$(git rev-parse --path-format=absolute --git-common-dir)/.." && pwd)
    git -C "$root" fetch --quiet origin main
    git -C "$root" worktree add --no-track -b "task/$name" "$root/.claude/worktrees/$name" origin/main
    echo "Worktree: $root/.claude/worktrees/$name (branch task/$name from origin/main)"

# Remove a task worktree. Its branch stays until the PR merges.
task-done $name:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ ! "$name" =~ ^[a-z0-9][a-z0-9._-]*$ ]]; then
      echo "task-done: the name must be lowercase letters, digits, '.', '_' or '-'" >&2
      exit 1
    fi
    root=$(cd "$(git rev-parse --path-format=absolute --git-common-dir)/.." && pwd)
    git -C "$root" worktree remove "$root/.claude/worktrees/$name"
    git -C "$root" worktree prune
    echo "Removed $root/.claude/worktrees/$name. Delete the branch after its PR merges: git branch -d task/$name"
