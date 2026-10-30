# Memory, experience and timing

Read by the `ux-memory` reviewer. Legend: **A** = automated gate (fails CI), **P** = partial (heuristic gate plus reviewer), **R** = reviewer judgement only. Paths are relative to the repo root.

## Serial position effect

People remember the first and last items best, so the most important items go first and last.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| SP-1 | Primary navigation: Agents first, Settings last. | `e2e/contract/app-shell/app-shell.contract.spec.ts` | A |
| SP-2 | Forms: required fields first, the commit action last. | Reading the screen | R |
| SP-3 | Lists default to "recently edited first". | Unit test on the default sort | R today; planned with the agent list, M2 |
| SP-4 | Summaries open with the outcome and end with the next step. | Reading the screen | R |

Reviewer checklist:
- [ ] A new navigation item goes between the first and last items, not at either end.
- [ ] Optional and advanced fields come after the required ones.
- [ ] Each list's default order puts what the user most likely wants at the top.

## Peak-end rule

People judge an experience by its high point and its ending, so the first successful test call and publishing are designed with care, and nothing ends in a dead end.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| PE-1 | The first successful test call ends on a summary: turns, latency p50/p95, transcript, and a "Publish" call to action. | `@golden` spec asserts the `role=status` text and the next-step CTA | Planned, M5 |
| PE-2 | Publishing ends on the version number, the live channel, a "Roll back" link and a success toast. | `@golden` spec asserts the end screen | Planned, M6 |
| PE-3 | Every error state ends with a recovery action. | `frontend/src/app/app.test.tsx` covers `RouteError` ("Try again") and `NotFound` ("Go to Agents") | P: A for these two, R for new error states |
| PE-4 | Peak and end screens look finished: no placeholder copy, clear success state. | Visual snapshots of the end states | R today; snapshots planned |

Reviewer checklist:
- [ ] Each flow's last screen says what happened and offers the next step.
- [ ] Errors explain what went wrong in plain words and offer a way forward.
- [ ] The test-call summary and the publish confirmation get extra polish (copy, layout, feedback).
- [ ] No flow ends on a blank screen, a spinner, or a raw error.

## Zeigarnik effect

People remember unfinished tasks, so setup progress stays visible until the agent is published.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| ZEIG-1 | Each agent shows a 4-step setup checklist (Provision, Plan, Test, Publish) with `role=progressbar` and `aria-valuenow` at 0, 25, 50, 75 or 100 %. | Playwright: new agent → 0 %; after Provision → 25 %; reload → unchanged | Planned, M2 |
| ZEIG-2 | Progress comes from the server and survives a reload. | Same Playwright test; `GET /agents/{id}/progress` API test | Planned, M2 |
| ZEIG-3 | The progress calculation is a pure, unit-tested function. | Unit test in `features/agents/state` | Planned, M2 |
| ZEIG-4 | An agent with changes not yet published shows an "Unpublished changes" badge. | E2E test on the badge | Planned, M6 |

Reviewer checklist:
- [ ] The next unfinished step is easy to find from the agent's page.
- [ ] Progress never moves backwards without a visible reason.
- [ ] Completed steps are marked done, not hidden.
