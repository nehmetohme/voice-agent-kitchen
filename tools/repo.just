# Repo-wide recipes: secrets, TOML, workflows, contracts, worktrees.

# Repo-wide gates
repo-verify: repo-verify-fast

# Fast repo-wide gates
repo-verify-fast:
    gitleaks dir . --no-banner --redact
    taplo fmt --check
