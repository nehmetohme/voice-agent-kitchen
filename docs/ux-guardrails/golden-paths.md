# Golden paths

The few flows that carry most of the product's use (Pareto). They get the most polish, the
strictest budgets and their own end-to-end tests. The `ux-cognitive` reviewer owns this list;
`ux-memory` checks the end states (peak-end).

Legend: **A** = automated gate (fails CI), **P** = partial (heuristic gate plus reviewer),
**R** = reviewer judgement only.

## The paths

The table is machine-read: `frontend/e2e/support/golden-paths.test.ts` parses every row whose
first cell is a backticked ID. Keep the columns in this order.

| ID | Path | Budget | End state (peak-end) | Milestone | Status |
|---|---|---|---|---|---|
| `create-agent` | Create an agent from the Agents page | ≤ 8 interactions, ≤ 60 s, ≤ 3 required fields | Agent page with "Saved" status and setup progress at 25 % | M2 | active |
| `edit-plan` | Edit the agent's plan in the flow editor | Autosave ≤ 1 s after the last edit | Plan valid, errors shown on their node, progress at 50 % | M3 | planned |
| `test-call` | Talk to the agent in the browser | Feedback ≤ 100 ms, live latency badge each turn | Summary: turns, latency p50/p95, transcript and a "Publish" call to action | M5 | planned |
| `publish` | Publish a version that passed its tests | ≤ 3 interactions from the test summary | Version number, live channel, "Roll back" link and a success toast | M6 | planned |
| `rollback` | Roll back to an earlier version | Confirmation in a `role=alertdialog` | Previous version live, audit event written, success toast | M6 | planned |

**Status** is `planned` or `active`. Set a path to `active` in the same PR that adds its spec.

## Rules

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| GOLD-1 | Every `active` path has a spec at `frontend/e2e/golden/<id>.golden.spec.ts` tagged `@golden`. | A | `e2e/support/golden-paths.test.ts` (Vitest, in `just verify`) |
| GOLD-2 | Every screen a golden spec visits calls `auditScreen(page)`. | P | Spec review; `auditScreen` itself is A |
| GOLD-3 | Each golden spec asserts its budget (interactions, time, or latency) and its end state from the table. | P | Spec review |
| GOLD-4 | Golden paths stay under INP ≤ 200 ms at 4× CPU throttle. | A (planned, M2) | Playwright Event Timing gate |
| GOLD-5 | Each end state has a visual snapshot. | A (planned, M2) | Playwright `toHaveScreenshot` |
| GOLD-6 | Each primary step emits one analytics event. | A (planned, M2) | Golden spec asserts the event |
| GOLD-7 | Polish goes to these paths first; a change that slows one needs a stated reason. | R | `ux-cognitive` reviewer |

## Writing a golden spec

```ts
// frontend/e2e/golden/create-agent.golden.spec.ts
import { expect, test } from '@playwright/test';

import { auditScreen } from '../support/ux-audit';

test('create an agent', { tag: '@golden' }, async ({ page }) => {
  await page.goto('/agents');
  await auditScreen(page);
  // ... at most 8 interactions ...
  await expect(page.getByRole('status')).toHaveText('Saved');
  await expect(page.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '25');
  await auditScreen(page);
});
```

## Reviewer checklist

- [ ] Does the change touch a golden path? If so, does its spec still assert the budget and end state?
- [ ] Is a new path being added because it is truly in the top 20 % of use, not because it is new?
- [ ] Does each end state give the user a clear next step (peak-end)?
- [ ] Did the status move to `active` in the same PR as the spec?
