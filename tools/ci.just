# CI and GitHub recipes: workflow linting, action pinning, rulesets, git hooks. See docs/ci.md.

# GitHub repository that `rulesets-apply` targets (owner/name)
ci_repo := env('VAK_GITHUB_REPO', 'nehmetohme/voice-agent-kitchen')

# Workflow gates (the CI part of `just verify`): actionlint, then zizmor
ci-verify: ci-verify-fast
    zizmor --no-progress .github

# Fast workflow gate (the CI part of `just verify-fast`): actionlint
ci-verify-fast:
    actionlint

# Scan the commits in base..head for secrets (git content, not the working tree). CI passes the PR or push range; with no usable base, only head is scanned
[positional-arguments]
ci-secrets base="" head="HEAD":
    #!/usr/bin/env bash
    set -euo pipefail
    base=${1:-}
    head=${2:-HEAD}
    if [[ -n "$base" && ! "$base" =~ ^0+$ ]] && git cat-file -e "$base^{commit}" 2>/dev/null; then
      range="$base..$head"
    else
      range="-1 $head"
    fi
    echo "gitleaks: scanning commits $range"
    gitleaks git --redact --no-banner --verbose --log-opts="$range" .

# Pin every action to a full commit SHA with a `# vX.Y.Z` comment (pinact, needs network)
ci-pin:
    GITHUB_TOKEN="${GITHUB_TOKEN:-$(gh auth token 2>/dev/null || true)}" pinact run --verify-comment

# Create or update the rulesets in .github/rulesets/ by name. Changes live settings: owner only. `just rulesets-apply true` prints the plan and diff without writing.
rulesets-apply dry_run="false":
    #!/usr/bin/env bash
    set -euo pipefail
    repo='{{ ci_repo }}'
    dry_run='{{ dry_run }}'
    # Keys starting with "_" are notes for readers; GitHub's API doesn't accept them.
    strip='walk(if type == "object" then with_entries(select(.key | startswith("_") | not)) else . end)'
    # The fields we manage, sorted, for comparing with what GitHub returns.
    shape='{name, target, enforcement, conditions, bypass_actors, rules} | walk(if type == "object" then del(.integration_id | select(. == null)) else . end)'
    existing=$(gh api --paginate "repos/$repo/rulesets?includes_parents=false" | jq -s 'add // []')
    for file in .github/rulesets/*.json; do
      body=$(jq "$strip" "$file")
      name=$(jq -r '.name' <<<"$body")
      id=$(jq -r --arg name "$name" 'map(select(.name == $name)) | first | .id // empty' <<<"$existing")
      if [ "$dry_run" = "true" ]; then
        if [ -z "$id" ]; then
          echo "would create ruleset '$name' from $file"
        else
          echo "would update ruleset '$name' (id $id) from $file; diff live -> file:"
          diff -u --label live --label "$file" \
            <(gh api "repos/$repo/rulesets/$id" | jq -S "$shape") \
            <(jq -S "$shape" <<<"$body") || true
        fi
      elif [ -z "$id" ]; then
        gh api --method POST "repos/$repo/rulesets" --input - <<<"$body" >/dev/null
        echo "created ruleset '$name' from $file"
      else
        gh api --method PUT "repos/$repo/rulesets/$id" --input - <<<"$body" >/dev/null
        echo "updated ruleset '$name' (id $id) from $file"
      fi
    done

# Install the git hooks from lefthook.yml. Run once per clone; linked worktrees share them.
setup:
    lefthook install
