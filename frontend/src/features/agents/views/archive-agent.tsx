import { useNavigate } from '@tanstack/react-router';
import { useId } from 'react';

import { ConfirmDialog } from '../../../shared/ui/confirm-dialog';
import { useArchiveAgent } from '../state/agent-mutations';

/**
 * Archiving, at the bottom of the page and away from everything else (FITTS-6), behind a
 * confirmation (VR-4). It ends on the Agents list.
 */
export function ArchiveAgent({ agentId, name }: { agentId: string; name: string }) {
  const navigate = useNavigate();
  const archive = useArchiveAgent(agentId);
  const titleId = useId();
  return (
    <section
      aria-labelledby={titleId}
      className="flex flex-col items-start gap-3 border-t border-line pt-6"
    >
      <h2 id={titleId} className="text-lg font-semibold">
        Archive
      </h2>
      <p className="text-muted">An archived agent leaves the list and can't be edited.</p>
      <ConfirmDialog
        triggerLabel="Archive agent"
        title={`Archive ${name}?`}
        description="It leaves the agent list and can no longer be edited."
        confirmLabel="Archive"
        pending={archive.isPending}
        onConfirm={() => {
          archive.mutate(undefined, {
            onSuccess: () => {
              void navigate({ to: '/agents' });
            },
          });
        }}
      />
      {archive.isError && (
        <p role="alert" className="text-danger">
          The agent wasn't archived. Check your connection, then try again.
        </p>
      )}
    </section>
  );
}
