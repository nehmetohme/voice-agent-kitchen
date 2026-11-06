# e2e/

Playwright tests against the production build (`pnpm build`, then `pnpm e2e`; `just
frontend-verify` does both). Chromium only. The web server is `vite preview`.

**The real backend.** `/api` is never mocked here. Playwright starts two web servers:
`support/backend-server.ts` (Postgres 18 in Docker with a throwaway in-memory database, then
`vak-server migrate` and `vak-server serve` in the local profile with the dev operator
`e2e@example.com` and the tool host allow-list `calendar.example.com,*.tools.example.com`) and `vite preview`, whose `/api` proxy points at it (`VAK_API_URL`). So
`pnpm e2e` needs Docker and the Rust toolchain; the first run builds `vak-server`. Stopping
the run removes the container.

**Models for test runs.** The e2e server's LLM and decision model are the deterministic fakes in
`support/fake-models.ts` (an OpenAI-compatible stream and System One), which
`backend-server.ts` starts on a free local port and points `vak-server` at (it drops the
developer's own `VAK_*` settings first). The real adapters talk to them, so test runs work end to
end and always end the same way. They are part of contract `test-runs` v1: never edit them once
tagged.

**The one exception: a voice call.** The e2e server has no speech models, so it refuses test calls.
A spec that needs a call fakes exactly that, and nothing else: `page.route` answers
`POST /api/v1/agents/{id}/sessions` with a media path, and `page.routeWebSocket` plays the
server on that media socket (vak.media.v1 events), with Chromium's fake microphone
(`--use-fake-device-for-media-stream`). See `contract/agent-workspace/` and
`golden/test-call.golden.spec.ts`, whose fake microphone plays the recorded utterance in
`backend/fixtures/audio/` (`--use-file-for-fake-audio-capture`).

**Order.** The agent-provisioning contract starts from an empty agent list, so it runs first
and alone (project `contract-empty-list`). The other contracts (`contract`) wait for it, and
every other spec (`chromium`) waits for them (`dependencies`). Running a later spec alone
(`--no-deps`) on a fresh database with several workers can fail its first agent creations
with 503: the server's first-sight operator upsert races. Use `--workers 1` there.

**Ports.** Parallel worktrees run their own servers, so each worktree derives its ports from a
hash of its path (`support/ports.ts`): web 4200–4999, API 14200–14999, database 24200–24999.
`E2E_PORT`, `E2E_API_PORT` and `E2E_DB_PORT` override them, e.g. `E2E_PORT=4175 just verify`.

| Folder                | What                                                                                             | Edit?                                              |
| --------------------- | ------------------------------------------------------------------------------------------------ | -------------------------------------------------- |
| `contract/<feature>/` | Contract specs, pinned by `contract/<feature>/vN` tags                                           | **Never** after tagging; add `vN+1`                |
| `golden/`             | One `<id>.golden.spec.ts` per golden path in `docs/ux-guardrails/golden-paths.md`                | Yes                                                |
| `support/`            | Shared helpers: `ux-audit.ts` (`auditScreen`), `ports.ts`, `backend-server.ts`, `fake-models.ts` | Yes, but `fake-models.ts` is pinned by `test-runs` |

## Rules

- **Call `auditScreen(page)` on every screen a test visits**, after it settles. It checks:
  ≤ 1 visible primary action, primary nav ≤ 7 links, targets ≥ 44 × 44 px, ≤ 4 actions per
  `[data-region="actions"]`, ≤ 7 inputs per fieldset, choices in named groups, and axe WCAG
  2.2 AA. A failure lists each finding with its law.
- Find elements by role and accessible name (`getByRole('button', { name: 'Publish' })`).
  No CSS selectors except `data-variant` and `data-region`, which are part of the UX contract.
- Tag tests: `@contract` for contract specs, `@golden` for golden paths.
- No fixed waits (`waitForTimeout`); wait for a role, text or URL.
- Golden specs assert their budget and end state from `golden-paths.md`. Marking a path
  `active` without its spec fails `e2e/support/golden-paths.test.ts` (Vitest).
- Voice tests use Chromium's fake media flags, with the WAV fixture where the audio matters;
  never a real mic. Set them with a top-level `test.use({ launchOptions })`: a worker option
  can't go in a `describe`.

## May import

`@playwright/test`, `@axe-core/playwright`, and `e2e/support/`. Never app source: test what
the user sees.

## Changing `auditScreen`

`ux-audit.ts` is a gate. Every new check gets a failing and a passing case in
`support/ux-audit.spec.ts`, and its rule ID in `docs/ux-guardrails/`. Never relax a limit to
make a screen pass; fix the screen.

## Example

```ts
import { expect, test } from '@playwright/test';

import { auditScreen } from '../support/ux-audit';

test('publish ends on the live version', { tag: '@golden' }, async ({ page }) => {
  await page.goto('/agents/agt_1/publish');
  await page.getByRole('button', { name: 'Publish' }).click();
  await expect(page.getByRole('status')).toContainText('Version 3 is live');
  await expect(page.getByRole('link', { name: 'Roll back' })).toBeVisible();
  await auditScreen(page);
});
```
