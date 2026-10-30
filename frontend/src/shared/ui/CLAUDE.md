# src/shared/ui/

The app's UI kit: shadcn-style components we own, built on Base UI primitives and styled with
the token theme. **The only folder that may import `@base-ui/react`** or render a raw
`<button>`.

| File                                                       | What                                                                                                                                                                |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `button.tsx`                                               | `Button`: variants `primary`, `secondary` (default), `ghost`, `danger`; `buttonLook(variant)` styles a router link as a button (Base UI: links keep link semantics) |
| `field.tsx`                                                | `Field` (label, control, help and error text tied by IDs) and `controlClassName`                                                                                    |
| `text-field.tsx`, `select-field.tsx`, `checkbox-field.tsx` | Labelled native controls; selects are for short fixed lists (≤ 7, HICK-3)                                                                                           |
| `progress-bar.tsx`                                         | `ProgressBar`: Base UI Progress, a named `role=progressbar` with `aria-valuenow`                                                                                    |
| `confirm-dialog.tsx`                                       | `ConfirmDialog`: a danger trigger and a Base UI `role=alertdialog` (VR-4)                                                                                           |
| `app-shell.tsx`                                            | `AppShell` (sidebar, top bar, main), `SidebarNav`, `SidebarNavItem`, `sidebarLinkClassName`                                                                         |
| `empty-state.tsx`                                          | `EmptyState`: title, description, optional icon and action                                                                                                          |
| `tokens.css`                                               | **Generated** Tailwind `@theme` from `tokens/vak.tokens.json`; run `pnpm tokens`                                                                                    |
| `*.stories.tsx`                                            | Storybook stories, next to their component                                                                                                                          |

## May import

- `@base-ui/react/*`, `class-variance-authority`, `lucide-react`, `react`, `src/shared/lib`.
- Never `features/`, `routes/`, `app/` or `shared/api` (`shared-not-to-upper-layers`).

## Conventions

- **Variants are fixed** (similarity). Add a variant only with a UX reason; never pass ad-hoc
  colour classes from outside.
- Every interactive component exposes `data-variant` when it has variants, and meets the
  **44 × 44 px** target (`min-h-target min-w-target`) in every variant (Fitts).
- **The primary colour is only for `Button variant="primary"`** (lint:
  `vak-ux/primary-colour-only-in-button`). Focus uses `outline-focus`.
- Style only with token classes. Arbitrary values (`p-[13px]`) fail lint; add a token instead.
  Off-token classes (`p-5`, `bg-red-500`) do not exist, because the default theme is reset.
- Wrap Base UI parts with `data-slot` names, as shadcn does. Keep a component ≤ 200 lines.
- Icons come from `lucide-react` and are `aria-hidden` unless they are the only label.
- To add a shadcn component, copy its Base UI variant, then swap colours and sizes for tokens.

## Testing

- `button.contract.test.tsx` is part of contract `app-shell/v1`: never edit it.
- Each component has a Testing Library test by role and name, or a story, or both.
- Stories show every variant and the disabled state.
- Coverage counts toward the 80 % frontend floor; stories are excluded.

## Example

```tsx
import { Button } from '../../../shared/ui/button';

<div data-region="actions" className="flex gap-2">
  <Button variant="primary" type="submit">
    Create agent
  </Button>
  <Button onClick={onCancel}>Cancel</Button>
</div>;
```
