import type { ReactNode } from 'react';

import { cn } from '../lib/cn';

/** A section link: a 44 px target, marked when it is the current section. */
export const sectionLinkClassName = cn(
  'flex min-h-target items-center rounded-md px-3 text-sm font-medium whitespace-nowrap text-muted',
  'transition-colors hover:bg-hover hover:text-ink',
  'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus',
  'aria-[current=page]:bg-selected aria-[current=page]:text-ink',
);

/**
 * The sections of one page, as a named navigation of links (Jakob: tabs that are links, so
 * each section has its own address and the back button works). Children are `<li>`s holding
 * router links styled with `sectionLinkClassName`.
 */
export function SectionNav({ label, children }: { label: string; children: ReactNode }) {
  return (
    <nav aria-label={label}>
      <ul className="flex flex-wrap gap-1 border-b border-line pb-2">{children}</ul>
    </nav>
  );
}
