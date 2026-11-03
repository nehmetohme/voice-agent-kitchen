import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';

import { ActionMenu } from '../../../shared/ui/action-menu';
import { ConfirmDialog } from '../../../shared/ui/confirm-dialog';
import { useArchiveAgent } from '../state/agent-mutations';

/**
 * The agent's rare actions, out of the main flow in a "More actions" menu (HICK-2): archiving is
 * a menu item and a confirmation away, never a button beside the primary action (FITTS-6). It
 * asks first (VR-4) and ends on the Agents list.
 */
export function AgentActions({ agentId, name }: { agentId: string; name: string }) {
  const navigate = useNavigate();
  const archive = useArchiveAgent(agentId);
  const [confirming, setConfirming] = useState(false);
  return (
    <div className="flex shrink-0 items-center gap-2">
      {archive.isError && (
        <p role="alert" className="text-sm text-danger">
          The agent wasn't archived. Check your connection, then try again.
        </p>
      )}
      <ActionMenu
        label="More actions"
        actions={[
          {
            label: 'Archive agent…',
            danger: true,
            onSelect: () => {
              setConfirming(true);
            },
          },
        ]}
      />
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title={`Archive ${name}?`}
        description="It leaves the agent list and can no longer be edited."
        confirmLabel="Archive"
        pending={archive.isPending}
        onConfirm={() => {
          archive.mutate(undefined, {
            onSuccess: () => {
              void navigate({ to: '/agents' });
            },
            onError: () => {
              setConfirming(false);
            },
          });
        }}
      />
    </div>
  );
}
