import { CircleAlert, CircleCheck } from 'lucide-react';

import { Button } from '../../../shared/ui/button';
import type { SaveStatus as Status } from '../state/autosave';

const TEXT: Record<Status, string> = {
  saved: 'Saved',
  unsaved: 'Unsaved changes',
  saving: 'Saving…',
  invalid: 'Not saved: fix the marked field',
  failed: 'Not saved',
};

/**
 * Whether every edit is stored (PARK-1). Announced politely; a failed save offers a retry, so
 * the state never ends in a dead end.
 */
export function SaveStatus({ status, onRetry }: { status: Status; onRetry: () => void }) {
  const problem = status === 'invalid' || status === 'failed';
  return (
    <div className="flex items-center gap-2">
      <output
        aria-live="polite"
        className={
          problem
            ? 'flex items-center gap-2 text-sm text-danger'
            : 'flex items-center gap-2 text-sm text-muted'
        }
      >
        {problem && <CircleAlert aria-hidden className="size-4" />}
        {status === 'saved' && <CircleCheck aria-hidden className="size-4 text-success" />}
        {TEXT[status]}
      </output>
      {status === 'failed' && (
        <Button variant="ghost" onClick={onRetry}>
          Try again
        </Button>
      )}
    </div>
  );
}
