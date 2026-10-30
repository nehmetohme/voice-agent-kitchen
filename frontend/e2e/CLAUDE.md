# e2e/

Playwright tests against the production build (`pnpm build`, then `pnpm e2e`; `just
frontend-verify` does both). Chromium only. The web server is `vite preview`.

**Ports.** Parallel worktrees run their own servers, so each worktree derives its ports from a
hash of its path (`support/ports.ts`): web 4200–4999, API 14200–14999, database 24200–24999.
`E2E_PORT`, `E2E_API_PORT` and `E2E_DB_PORT` override them, e.g. `E2E_PORT=4175 just verify`.

| Folder                | What                                                                              | Edit?                               |
| --------------------- | --------------------------------------------------------------------------------- | ----------------------------------- |
| `contract/<feature>/` | Contract specs, pinned by signed `contract/<feature>/vN` tags                     | **Never** after tagging; add `vN+1` |
| `golden/`             | One `<id>.golden.spec.ts` per golden path in `docs/ux-guardrails/golden-paths.md` | Yes                                 |
| `support/`            | Shared helpers: `ux-audit.ts` (`auditScreen`), fixtures, page helpers             | Yes                                 |

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
- Voice tests (M5) use Chromium's fake media flags with a WAV fixture; never a real mic.

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
