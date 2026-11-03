import { Popover } from '@base-ui/react/popover';
import type { ReactNode } from 'react';

import { Button } from './button';

interface DetailsPopoverProps {
  /** The short visible text of the button that opens it, such as "Details". */
  triggerLabel: string;
  /** The popover's heading; the button's accessible name is "<title> details". */
  title: string;
  children: ReactNode;
}

/**
 * Details on demand that never move the page: a quiet button that opens a Base UI popover
 * (`role=dialog`, named by its title) above it. Esc or a click outside closes it, and focus
 * returns to the button.
 */
export function DetailsPopover({ triggerLabel, title, children }: DetailsPopoverProps) {
  return (
    <Popover.Root>
      <Popover.Trigger
        data-slot="popover-trigger"
        render={<Button variant="ghost" aria-label={`${title} ${triggerLabel.toLowerCase()}`} />}
      >
        {triggerLabel}
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Positioner data-slot="popover-positioner" side="top" align="end" sideOffset={8}>
          <Popover.Popup
            data-slot="popover-popup"
            className="flex w-dvw max-w-readable flex-col gap-3 rounded-md border border-line bg-canvas p-4 text-ink shadow-md"
          >
            <Popover.Title className="font-semibold">{title}</Popover.Title>
            {children}
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  );
}
