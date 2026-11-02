import { Lock } from 'lucide-react';

import type { LockedText } from '../state/models';

/** The rule pack's locked texts, read-only: what every call opens with, word for word. */
export function LockedOpening({ locked }: { locked: readonly LockedText[] }) {
  if (locked.length === 0) return null;
  return (
    <figure className="flex flex-col gap-1 rounded-md border border-line bg-surface p-3">
      <figcaption className="flex items-center gap-2 text-sm font-medium text-muted">
        <Lock aria-hidden className="size-4" />
        Locked by the rule pack
      </figcaption>
      <p className="text-sm">{locked.map(({ text }) => text).join(' ')}</p>
    </figure>
  );
}
