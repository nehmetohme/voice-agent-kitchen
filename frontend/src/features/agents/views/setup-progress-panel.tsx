import { useQuery } from '@tanstack/react-query';

import { LoadFailed, Loading } from '../../../shared/ui/load-states';
import { agentProgressQueryOptions } from '../state/agents-queries';
import { toChecklist } from '../state/setup-checklist';
import { SetupChecklist } from './setup-checklist';

/** Setup progress first (Zeigarnik), so the next step is always in view. */
export function SetupProgressPanel({ agentId }: { agentId: string }) {
  const progress = useQuery(agentProgressQueryOptions(agentId));
  if (progress.isPending) return <Loading label="Loading setup progress…" />;
  if (progress.isError) {
    return (
      <LoadFailed
        what="setup progress"
        onRetry={() => {
          void progress.refetch();
        }}
      />
    );
  }
  return <SetupChecklist agentId={agentId} checklist={toChecklist(progress.data)} />;
}
