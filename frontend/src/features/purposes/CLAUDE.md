# src/features/purposes/

What agents are for and how they may work (milestone M3b, design in `docs/plan/purposes.md`).
The rules for every feature are in `../CLAUDE.md`; this file covers what is particular here.

- **List** (`views/purposes-overview.tsx`): the six built-in purposes first, then the
  workspace's, each with its version state and "Used by N agents". One primary action, "New
  purpose", copies a purpose (`views/copy-purpose-dialog.tsx`).
- **Page** (`views/purpose-page.tsx`): a workspace that fits the screen from `lg`, like the
  agent page. `PurposeHeader` shows the name, the version state (`state/version-state.ts`),
  "Create agent", "Duplicate" and the one primary "Save version N", with the reason when it has
  to wait. The section links (`?section=`, `state/sections.ts`) lead to Overview, Flow, Tools,
  Guardrails and Compliance; the section scrolls on its own.
- **Autosave** (`state/use-purpose-draft.ts`): each section is edited whole and only the
  changed sections are sent, 500 ms after the last edit. A built-in purpose is read-only and
  never sent.
- **Pure edits** (`state/purpose-edits.ts`): what a save sends, built-in tools (the opt-out tool
  stays on), HTTP tools and their check before they are added, hosts against the allow-list,
  escalations.
- **Flow:** the route composes the plans feature's `PurposeFlow` into the Flow section (the
  `flow` slot), so this feature never imports another one.
- **Limits** come from the generated schemas (`api/request-limits.ts`); the few the schema
  doesn't carry are the server's, and its answer names them.

## The agents feature and purposes

The Purpose pickers (`agents/state/use-purpose-choices.ts`) list every purpose with a saved
version, built-in ones first, through `ChoiceField`: a select up to seven, a searchable combobox
past seven (HICK-3). A built-in purpose is sent as its use case, another as `purposeId`.
`AgentHeader` shows "Update available" (`agents/views/purpose-update.tsx`) when the agent pins
an older version: the dialog lists what changes and what happens to the plan, and an edited
plan is kept unless the operator chooses to replace it.

## Testing

`src/test/purposes-api.ts` (purposes and the tool catalog) and `src/test/agent-purpose-api.ts`
(an agent's purpose and its update) are the in-memory fakes; `api/` tests use the generated
mocks. The end-to-end contract is `e2e/contract/purposes/` (`purposes` v1).
