# Interaction and ergonomics

Read by the `ux-interaction` reviewer. Legend: **A** = automated gate (fails CI), **P** = partial (heuristic gate plus reviewer), **R** = reviewer judgement only. Paths are relative to the repo root.

## Fitts's law

Time to hit a target grows with its distance and shrinks with its size, so targets are large and placed where the user is already working.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| FITTS-1 | Interactive targets are at least 44 × 44 CSS px. Inline text links are exempt. | `auditScreen(page)` measures `button`, `[role=button\|tab\|checkbox\|radio\|switch\|menuitem]`, `input[type=checkbox\|radio]` (its label counts) and `nav a[href]`; axe `target-size` (WCAG 2.5.8, 24 px floor) | A |
| FITTS-2 | Target size comes from the `spacing.target` token (44 px), not from ad-hoc sizes. | `frontend/tokens/tokens.test.ts` (`spacing.target` ≥ 44 px); `frontend/src/shared/ui/button.contract.test.tsx` (44 px through token classes) | A |
| FITTS-3 | Every button is the `shared/ui` `Button`, which carries the target size. | Oxlint `vak-ux/no-raw-button` (off only in `frontend/src/shared/ui`) | A |
| FITTS-4 | The commit button sits inside the form or panel it commits, at its end. | Planned: `auditScreen` asserts every `form` contains a `button[type=submit]`. Placement within the form stays reviewer judgement. | R today; check planned with the first form, M2 |
| FITTS-5 | Node actions sit on the node's own toolbar in the flow editor, not in a global toolbar. | Reading the screen | R (M3) |
| FITTS-6 | A destructive action is not placed next to the primary action. | Reading the screen | R |
| FITTS-7 | Frequent actions sit close to the content they act on (for example "End call" next to the live transcript). | Reading the screen | R |

Reviewer checklist:
- [ ] The next action after a step is near where the user's attention ends (bottom of a form, end of a row).
- [ ] No small icon-only targets; icon buttons still use the `Button` 44 px minimum.
- [ ] Destructive and primary actions are separated by distance or a menu.
- [ ] Pointer travel on golden paths is short: no trip to a far toolbar for a per-item action.
- [ ] Keyboard focus order follows the visual order.

## Doherty threshold

People stay engaged when the system answers within 400 ms, so every input gets immediate feedback and results arrive fast or show progress.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| DOHERTY-1 | Visual feedback within 100 ms of any input (pressed state, `aria-busy`, skeleton). | Test with MSW `delay(1000)` asserting `aria-busy` or a skeleton appears within 100 ms | R today; planned, M2 |
| DOHERTY-2 | The result, or a progress indicator, within 400 ms. | Same MSW delay test | R today; planned, M2 |
| DOHERTY-3 | Lab INP ≤ 200 ms on golden paths under 4× CPU throttle. | Playwright Event-Timing gate | Planned with the first golden path, M2 |
| DOHERTY-4 | Initial JS ≤ 200 kB gzip. | `size-limit` with `frontend/.size-limit.json` | A |
| DOHERTY-5 | Edits that rarely fail update optimistically; the live test call shows a latency badge per turn. | Reading the code and the screen | R (badge in M5) |
| DOHERTY-6 | Lighthouse user-flow trend (TBT, INP) on golden paths. | Nightly Lighthouse 13 user flows | Planned, CI task (trend, not a gate) |

Reviewer checklist:
- [ ] Every mutation shows pending state at once (disabled `Button` with `aria-busy`, inline spinner, or optimistic value).
- [ ] Long operations (test runs, publish) show progress, not a frozen screen.
- [ ] New routes are code-split if they add weight to the initial bundle.
- [ ] No synchronous heavy work on input handlers (parsing, large re-renders).
