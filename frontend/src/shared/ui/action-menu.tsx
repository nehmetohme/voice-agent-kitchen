import { Menu } from '@base-ui/react/menu';
import { Ellipsis } from 'lucide-react';

import { cn } from '../lib/cn';
import { Button } from './button';

export interface MenuAction {
  /** End it with "…" when the action asks for confirmation first. */
  label: string;
  onSelect: () => void;
  /** Destructive: shown in the danger colour. Confirm it in a `ConfirmDialog` (VR-4). */
  danger?: boolean | undefined;
}

/**
 * Rarely used actions behind one icon button (Hick, HICK-2): a Base UI menu, so arrow keys,
 * type-ahead and Esc work, and focus returns to the button. Every item is a 44 px target.
 */
export function ActionMenu({ label, actions }: { label: string; actions: readonly MenuAction[] }) {
  return (
    <Menu.Root>
      <Menu.Trigger data-slot="menu-trigger" render={<Button variant="ghost" aria-label={label} />}>
        <Ellipsis aria-hidden className="size-4" />
      </Menu.Trigger>
      <Menu.Portal>
        <Menu.Positioner data-slot="menu-positioner" align="end" sideOffset={4}>
          <Menu.Popup
            data-slot="menu-popup"
            className="flex flex-col rounded-md border border-line bg-canvas p-1 text-ink shadow-md"
          >
            {actions.map((action) => (
              <Menu.Item
                key={action.label}
                data-slot="menu-item"
                onClick={action.onSelect}
                className={cn(
                  'flex min-h-target min-w-target cursor-pointer items-center rounded-sm px-3 text-sm font-medium whitespace-nowrap',
                  'outline-none data-highlighted:bg-hover',
                  action.danger === true ? 'text-danger' : 'text-ink',
                )}
              >
                {action.label}
              </Menu.Item>
            ))}
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}
