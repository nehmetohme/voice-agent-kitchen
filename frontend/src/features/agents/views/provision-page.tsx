import { useQuery } from '@tanstack/react-query';

import { agentProgressQueryOptions, agentQueryOptions } from '../state/agents-queries';
import { isNotFound } from '../state/models';
import { toChecklist } from '../state/setup-checklist';
import { AgentSettingsForm } from './agent-settings-form';
import { ArchiveAgent } from './archive-agent';
import { AgentMissing, LoadFailed, Loading } from './load-states';
import { SetupChecklist } from './setup-checklist';

/** Setup progress first (Zeigarnik), so the next step is always in view. */
function SetupProgressSection({ agentId }: { agentId: string }) {
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
  return <SetupChecklist checklist={toChecklist(progress.data)} />;
}

/**
 * The Provision step of one agent: its setup progress, its settings (autosaved) and, last,
 * archiving.
 */
export function ProvisionPage({ agentId }: { agentId: string }) {
  const agent = useQuery(agentQueryOptions(agentId));
  if (agent.isPending) return <Loading label="Loading the agent…" />;
  if (agent.isError) {
    if (isNotFound(agent.error)) return <AgentMissing />;
    return (
      <LoadFailed
        what="this agent"
        onRetry={() => {
          void agent.refetch();
        }}
      />
    );
  }
  if (agent.data.archived) return <AgentMissing />;
  return (
    <div className="flex max-w-readable flex-col gap-8">
      <SetupProgressSection agentId={agentId} />
      <AgentSettingsForm key={agent.data.id} agent={agent.data} />
      <ArchiveAgent agentId={agentId} name={agent.data.settings.name} />
    </div>
  );
}
