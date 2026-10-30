# docs/ux-guardrails

The UX laws from `PROMPT.md` §8, written as measurable rules. The five UX reviewer agents read these checklists on every frontend change.

## Responsibility

- One file per reviewer group. Each law has stable rule IDs, how each rule is checked, and a short reviewer checklist.
- [golden-paths.md](golden-paths.md) lists the golden paths (Pareto). `frontend/e2e/support/golden-paths.test.ts` reads it.
- Sources: [research 03 §4](../research/03-frontend.md) and [quality.md §5](../plan/quality.md).

## File ↔ reviewer map

The file names are a contract with the reviewer agents in `.claude/agents/`. Do not rename or split them.

| File | Reviewer | Laws |
|---|---|---|
| [cognitive.md](cognitive.md) | `ux-cognitive` | Hick, Miller, Occam's razor, Pareto |
| [interaction.md](interaction.md) | `ux-interaction` | Fitts (incl. target distance), Doherty threshold |
| [perception.md](perception.md) | `ux-perception` | Jakob, proximity, similarity, uniform connectedness, Prägnanz, Von Restorff |
| [memory.md](memory.md) | `ux-memory` | Serial position, peak-end, Zeigarnik |
| [complexity.md](complexity.md) | `ux-complexity` | Tesler, Postel, Parkinson |
| [golden-paths.md](golden-paths.md) | all five, and CI | The paths that get `@golden` specs |

## Legend

- **A**: automated gate. It fails `just verify` and CI.
- **P**: partial. A heuristic gate catches the common case; the reviewer judges the rest.
- **R**: reviewer judgement only.
- **Planned, <milestone>**: the gate is designed but not built yet. Until it lands, the reviewer checks the rule by hand.

The automated gates today:
- `auditScreen(page)` in `frontend/e2e/support/ux-audit.ts`, self-tested in `ux-audit.spec.ts`
- the Oxlint `vak-ux` plugin in `frontend/tools/lint-rules/`
- the token theme and `frontend/tokens/tokens.test.ts`
- the Button and app shell contract tests
- dependency-cruiser, knip and size-limit

## Conventions

- **Every rule names its check.** A rule without a check is a wish; write "R" and say what the reviewer looks at.
- **Rule IDs are stable.** Reviewers cite them in findings (`FITTS-1`, `VR-2`). Never reuse a retired ID; mark the rule "retired" instead.
- **Measurable first.** Prefer a number (44 px, 7 items, 400 ms) over an adjective.
- **Be honest about status.** Mark a gate A only when it runs in `just verify` today.
- Keep each file under about 150 lines. Plain, short sentences.

## Changing a rule

1. Edit the rule in its file. Keep the ID if the intent is the same; add a new ID if it is not.
2. When a rule becomes automated, add the gate (test, lint rule or `auditScreen` check) **and** change "Checked by" to A in the same PR.
3. A gate that fails on purpose (canary) should prove the new check works before it is relied on.
4. If a limit changes (for example, the target size), change the token or constant and this file together.

## Testing expectations

Docs only. The `docs` reviewer checks that rule IDs, paths and gate names match the code. `golden-paths.test.ts` fails if an `active` path in [golden-paths.md](golden-paths.md) has no `@golden` spec.

## Example

A reviewer finding cites the rule and the evidence:

```json
{ "severity": "major", "file": "frontend/src/features/agents/views/agent-list.tsx", "line": 42,
  "message": "VR-1: two primary buttons on /agents ('Create agent', 'Import'). Make 'Import' secondary." }
```
