# Visual perception and layout

Read by the `ux-perception` reviewer. Legend: **A** = automated gate (fails CI), **P** = partial (heuristic gate plus reviewer), **R** = reviewer judgement only. Paths are relative to the repo root.

## Jakob's law

Users spend most of their time in other products, so this one follows the patterns they already know.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| JAKOB-1 | The standard shell: left sidebar, top bar, main content. A Cmd/Ctrl-K palette comes later. | `e2e/contract/app-shell/app-shell.contract.spec.ts` | A (palette planned) |
| JAKOB-2 | Only `shared/ui` primitives: no custom dropdowns or dialogs; only `shared/ui` imports Base UI. | dependency-cruiser `base-ui-only-in-shared-ui`; Oxlint `vak-ux/no-raw-button`; lint ban on raw `<select>` and `<dialog>` | A for Base UI and buttons; `<select>`/`<dialog>` ban planned |
| JAKOB-3 | Esc closes overlays; Enter submits forms; Cmd/Ctrl-K opens the palette. | Playwright keyboard tests | Planned with the first overlay and form, M2 |
| JAKOB-4 | The current section is marked in the navigation. | App shell contract asserts `aria-current="page"` | A |
| JAKOB-5 | Terminology matches comparable products (agent, version, publish, roll back, transcript). | Reading the copy against [research 04 §3](../research/04-compliance-and-market.md) | R |

Reviewer checklist:
- [ ] New screens reuse the shell and existing `shared/ui` patterns instead of inventing layouts.
- [ ] Labels use the words comparable voice-agent products use.
- [ ] Standard keys work as users expect (Esc, Enter, Tab order).
- [ ] Links look like links and buttons look like buttons.

## Proximity

Things close together read as one group, so spacing shows structure.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| PROX-1 | Spacing comes only from the 4-point token scale. | Token theme resets Tailwind (`--*: initial` in `frontend/src/shared/ui/tokens.css`), so off-scale classes do not exist; Oxlint `vak-ux/no-arbitrary-values` bans `p-[13px]`, `bg-(--x)`; `tokens.test.ts` checks the 4-point grid | A |
| PROX-2 | Gap inside a group ≤ 8 px (`spacing-2`); gap between groups ≥ 24 px (`spacing-6`). | Visual snapshots | R today; snapshots planned |
| PROX-3 | No inline `style` margin or padding. | Lint ban | R today; planned |
| PROX-4 | A label sits next to its field; help and error text sit directly under it. | Reading the screen | R |

Reviewer checklist:
- [ ] Related controls are visibly closer to each other than to unrelated ones.
- [ ] Section spacing is consistent across screens.
- [ ] No `oxlint-disable` for `vak-ux/no-arbitrary-values` without a reason in the comment.

## Similarity

Things that look alike are read as alike, so one meaning gets one look everywhere.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| SIM-1 | One `Button` with fixed variants: `primary`, `secondary`, `ghost`, `danger`. | Oxlint `vak-ux/no-raw-button`; `frontend/src/shared/ui/button.contract.test.tsx` | A |
| SIM-2 | Status colour uses semantic tokens only (`success`, `warning`, `danger`); no raw hex colours. | Token theme reset (non-token colour classes do not exist); lint ban on hex colours in TSX | A for classes; hex ban planned |
| SIM-3 | One icon set: `lucide-react`. | Lint ban on non-lucide icon imports | R today; planned |
| SIM-4 | The same entity gets the same badge, icon and label everywhere. | Storybook visual diffs | R today; planned |

Reviewer checklist:
- [ ] A new variant or badge is not a near-duplicate of an existing one.
- [ ] Each status has one colour and one label across screens.
- [ ] Icons for the same concept match across screens.

## Uniform connectedness

Elements inside one container or joined by a line read as related.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| UC-1 | Every radio, and every set of 2+ checkboxes with one name, sits in a named group (`fieldset` + `legend`, or `role=group`/`radiogroup` with a name). | `auditScreen` | A |
| UC-2 | Related controls share a container with an accessible name (`fieldset`, `role=group`, a `section` with `aria-labelledby`). | axe `aria-*` rules in `auditScreen` | P |
| UC-3 | Flow nodes are joined by visible edges; stepper steps are joined by a connector. | Reading the screen | R (M2 stepper, M3 flow editor) |

Reviewer checklist:
- [ ] Groups have a visible boundary or heading as well as an accessible name.
- [ ] Nothing unrelated sits inside a group's container.

## Prägnanz

People read the simplest shape first, so the visual vocabulary stays small and clear.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| PRAG-1 | At most 3 radius tokens, 3 elevation (shadow) tokens, 2 font families, 6 type sizes. | `frontend/tokens/tokens.test.ts` | A |
| PRAG-2 | No decorative gradients. | `tokens.test.ts` (no gradient tokens); lint ban on `bg-gradient-*`/`bg-linear-*` outside an allowlist | A for tokens; class ban planned |
| PRAG-3 | One visual idea per component; no ornament that carries no meaning. | Reading the screen | R |

Reviewer checklist:
- [ ] A new token is needed, and the token limits still hold.
- [ ] Shapes, borders and shadows are used only to show structure or state.

## Von Restorff effect

The one item that differs is the one remembered, so the main action stands out and nothing else competes with it.

| # | Rule (measurable) | Check | Checked by |
|---|---|---|---|
| VR-1 | At most 1 visible primary call to action; exactly 1 when the screen has a main action. | `auditScreen` (≤ 1); app shell contract | A for ≤ 1; R for "exactly 1" |
| VR-2 | The primary colour is reserved for the primary `Button` (focus uses its own `focus` token). | Oxlint `vak-ux/primary-colour-only-in-button` (allowed only in `frontend/src/shared/ui/button.tsx`) | A |
| VR-3 | `Button` is not primary by default. | `button.contract.test.tsx` | A |
| VR-4 | Destructive actions use the `danger` variant and confirm in a `role=alertdialog`. | Archive opens a `role=alertdialog` (`ConfirmDialog`; Testing Library in `features/agents/views`); Playwright for rollback | P: archive tested; rollback planned, M6 |

Reviewer checklist:
- [ ] The primary action is the one most users want next.
- [ ] Nothing else on the screen (colour, size, motion) competes with the primary action.
- [ ] Destructive actions are visually distinct and never primary.
