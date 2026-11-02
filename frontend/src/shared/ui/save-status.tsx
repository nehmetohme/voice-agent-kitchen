import { CircleAlert, CircleCheck } from 'lucide-react';

import { Button } from './button';

/**
 * - `saved`: everything on the page is stored.
 * - `unsaved`: an edit is waiting to be saved.
 * - `saving`: a save is in flight.
 * - `invalid`: a field must be fixed before anything can be saved.
 * - `failed`: the last save failed; editing again, or retrying, saves again.
 */
export type SaveState = 'saved' | 'unsaved' | 'saving' | 'invalid' | 'failed';

const TEXT: Record<SaveState, string> = {
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
export function SaveStatus({ status, onRetry }: { status: SaveState; onRetry: () => void }) {
  const problem = status === 'invalid' || status === 'failed';
  return (
    <div className="flex items-center gap-2">
      <output
        aria-label="Save status"
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
