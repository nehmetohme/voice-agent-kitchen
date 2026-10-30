# System complexity and behaviour

Read by the `ux-complexity` reviewer. Legend: **A** = automated gate (fails CI), **P** = partial (heuristic gate plus reviewer), **R** = reviewer judgement only. Paths are relative to the repo root.

## Tesler's law

Every system has complexity that cannot be removed; the system absorbs it so the user does not have to.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| TES-1 | Creating an agent requires at most 3 fields (name, purpose, voice). Every other field has a server default (language, model, turn-detection timings). | `backend/crates/api/tests/contract_agent_provisioning_api.rs` checks the OpenAPI `POST /api/v1/agents` body: ≤ 3 `required` properties, a `default` on every other one | A |
| TES-2 | An agent can be created with only the required fields. | `frontend/e2e/golden/create-agent.golden.spec.ts` and the agent-provisioning contract spec, against the real server | A |
| TES-3 | At most 1 "Advanced" disclosure per form. | `auditScreen` disclosure count | R today; planned, M2 |
| TES-4 | The system derives what it can (defaults, normalisation, inferred settings) instead of asking. | Reading the screen and the API | R |

Reviewer checklist:
- [ ] Each new required field is truly needed before anything can work.
- [ ] Defaults are sensible for the most common use case and visible, not hidden.
- [ ] Technical settings (models, timings, codecs) live behind the single "Advanced" disclosure.
- [ ] Error messages never ask the user to fix something the system could fix.

## Postel's law

Be liberal in what you accept and strict in what you send: forgiving input, validated output.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| POS-1 | Accept any phone format and normalise to E.164; trim and case-fold enum input; accept pasted scripts with mixed line endings. | Property-based tests (fast-check) on the normalisers in `frontend/src/shared/lib` | Planned with the first normaliser, M2 |
| POS-2 | Malformed URL search params fall back to defaults; the page renders without the error boundary. | TanStack Router `validateSearch` with a schema fallback; Playwright visits routes with bad params | Planned with the first route that takes search params |
| POS-3 | Every response is Zod-parsed on receipt; a body that breaks the contract fails loudly. | `frontend/src/shared/api/http.ts` raises `ContractViolationError`; `http.test.ts` | A (applies to generated calls once orval passes schemas, T0.4) |
| POS-4 | Every request body is Zod-parsed before it is sent. | Unit test that `http.ts` rejects an invalid payload | Gap: not wired yet; planned, T0.4 |
| POS-5 | An unknown address shows a not-found page with a way back, not a crash. | `frontend/src/app/app.test.tsx` | A |

Reviewer checklist:
- [ ] Inputs accept the formats people actually type or paste.
- [ ] Normalisation happens once, in `shared/lib` or the backend, not in each view.
- [ ] Data sent to the backend is validated against the contract schema.
- [ ] Invalid input gets a specific, fixable message next to the field.

## Parkinson's law

Work expands to fill the time it is given, so tasks are short, defaults are smart and saving is automatic.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| PARK-1 | Drafts autosave within 1 s of the last edit and show a visible "Saved" status; no Save button is needed. | `frontend/e2e/contract/agent-provisioning/`: edit → not "Saved" → "Saved" within 2 s → reload → value persists; no Save button | A |
| PARK-2 | The create-agent golden path takes at most 8 interactions and 60 s. | Interaction counter and clock in the `@golden` create-agent spec and the agent-provisioning contract spec | A |
| PARK-3 | Templates exist for common jobs: support, booking, survey, outbound. | Domain tests that each template produces a valid plan; usefulness is R | Planned, M3 |
| PARK-4 | A task can end as soon as its required parts are done; optional work does not block. | Reading the flow | R |

Reviewer checklist:
- [ ] No step makes the user wait or click "Save" to keep their work.
- [ ] Each flow has a clear finish line and does not invite endless tweaking.
- [ ] Starting from a template is the default path for new agents.
