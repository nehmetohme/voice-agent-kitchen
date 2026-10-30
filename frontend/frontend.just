# frontend recipes, imported by the root justfile. Every recipe runs in frontend/.
# Tools run through pnpm, so they use the Node version pinned in package.json (devEngines.runtime).

# Install dependencies from the lockfile and the Playwright browser
[working-directory('frontend')]
frontend-install:
    pnpm install --frozen-lockfile
    pnpm exec playwright install chromium

# Start the dev server on :5173 (proxies /api to the Rust server on :3000)
[working-directory('frontend')]
frontend-dev: frontend-install
    pnpm run dev

# All frontend gates
[working-directory('frontend')]
frontend-verify: frontend-install
    pnpm run fmt:check
    pnpm run lint
    pnpm run typecheck
    pnpm run depcruise
    pnpm run knip
    pnpm run test:coverage
    pnpm run build
    pnpm run size
    pnpm run storybook:build
    pnpm run e2e

# Fast frontend gates: format, lint, types, boundaries
[working-directory('frontend')]
frontend-verify-fast: _frontend-deps
    pnpm run fmt:check
    pnpm run lint
    pnpm run typecheck
    pnpm run depcruise

# Format and lint one file, for the post-edit hook (path absolute, or relative to the repo root or frontend/)
[positional-arguments]
[working-directory('frontend')]
frontend-check-file file:
    #!/usr/bin/env bash
    set -euo pipefail
    path="${1#frontend/}"
    pnpm exec oxfmt --no-error-on-unmatched-pattern "$path"
    pnpm exec oxlint --no-error-on-unmatched-pattern "$path"

[private]
[working-directory('frontend')]
_frontend-deps:
    pnpm install --frozen-lockfile --prefer-offline
