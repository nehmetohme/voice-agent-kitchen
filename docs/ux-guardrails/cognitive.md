# Cognitive load and decisions

Read by the `ux-cognitive` reviewer. Legend: **A** = automated gate (fails CI), **P** = partial (heuristic gate plus reviewer), **R** = reviewer judgement only. Paths are relative to the repo root.

## Hick's law

Every extra choice slows the decision, so each step offers few choices and one obvious next action.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| HICK-1 | At most 1 visible primary action per screen. | `auditScreen(page)` counts visible `[data-variant="primary"]` | A |
| HICK-2 | At most 4 actions (1 primary + 3 secondary) per action group. More actions go into a "More" menu. | `auditScreen` counts buttons and links in each `[data-region="actions"]` | A for marked groups; R that groups are marked |
| HICK-3 | A choice list with more than 7 options is a searchable Combobox, not a Select or radio group. | Lint rule on `<Select>` with > 7 static options | R today; lint planned, M2 |
| HICK-4 | Each step of Provision → Plan → Test → Publish asks one main question at a time. | Reading the screen | R |

Reviewer checklist:
- [ ] Each action group that renders buttons carries `data-region="actions"`, so HICK-2 can see it.
- [ ] Secondary actions that are rarely used sit in a "More" menu, not beside the primary.
- [ ] Long option lists use a Combobox with search.
- [ ] No screen asks the user to decide something the system could default (see TES-4).

## Miller's law

People hold about 7 ± 2 items in working memory, so lists, forms and identifiers come in small chunks.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| MILLER-1 | Primary navigation has at most 7 links. | `auditScreen` counts links in `navigation "Primary"`; `e2e/contract/app-shell/app-shell.contract.spec.ts` asserts 2–7 | A |
| MILLER-2 | At most 7 inputs per `fieldset` or form section. | `auditScreen` counts form controls per `fieldset` | A inside fieldsets; R for inputs outside one |
| MILLER-3 | Tables show at most 7 columns by default; more through a column picker. | `auditScreen` column count | R today; planned with the first table, M2 |
| MILLER-4 | Phone numbers and long IDs display chunked: `+1 415 555 0100`. | Unit tests on the formatters in `frontend/src/shared/lib` | R today; planned with the first formatter, M2 |

Reviewer checklist:
- [ ] Long forms are split into `fieldset`s with a `legend`, each of 7 inputs or fewer.
- [ ] Lists and summaries group items under headings instead of one long run.
- [ ] Identifiers and numbers are chunked or abbreviated for display, with the full value on copy.

## Occam's razor

The simplest design that works wins: fewer components, fewer props, fewer dependencies.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| OCCAM-1 | A source file has at most 200 lines, not counting blank lines and comments. Tests are exempt. | Oxlint `max-lines` | A |
| OCCAM-2 | Cyclomatic complexity is at most 10 per function. | Oxlint `complexity` | A |
| OCCAM-3 | A function takes at most 4 positional parameters; a component takes at most 8 props. | Oxlint `max-params` (parameters) | A for parameters; R for props |
| OCCAM-4 | No unused files, exports or dependencies. | `knip` | A |
| OCCAM-5 | No new dependency when `shared/` or an existing package already covers the need; no new primitive when `shared/ui` has one. | Reading the diff, with the `dependencies` reviewer | R |

Reviewer checklist:
- [ ] The change adds the fewest new concepts (components, stores, props) that solve the problem.
- [ ] Nothing is built "for later" without a current caller.
- [ ] A new component is not a near-copy of one in `frontend/src/shared/ui`.
- [ ] Configuration options exist only where users actually differ.

## Pareto principle

About 20% of the features carry 80% of the use. Polish goes to the golden paths first.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| PARETO-1 | Each golden path in [golden-paths.md](golden-paths.md) has an `@golden` Playwright spec once its milestone marks it `active`. | `frontend/e2e/support/golden-paths.test.ts` fails if an `active` path has no `frontend/e2e/golden/<id>.golden.spec.ts` tagged `@golden` | A |
| PARETO-2 | Each golden path has a performance budget (INP ≤ 200 ms) and visual snapshots of its key screens. | Event-Timing gate; Playwright `toHaveScreenshot` | Planned, M2 |
| PARETO-3 | Every primary step of a golden path emits an analytics event. | Unit or E2E test on the event | Planned, M2 |
| PARETO-4 | Polish (copy, empty states, loading states, keyboard) goes to golden-path screens before other screens. | Reading the plan and the diff | R |

Reviewer checklist:
- [ ] A change on a golden path keeps its `@golden` spec passing and, if it changes the flow, updates [golden-paths.md](golden-paths.md).
- [ ] Effort spent on rare screens does not come at the cost of a golden-path screen.
- [ ] A new feature that should be a golden path is added to [golden-paths.md](golden-paths.md).
