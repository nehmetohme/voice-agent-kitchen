import { useQuery } from '@tanstack/react-query';

import { LoadFailed, Loading } from '../../../shared/ui/load-states';
import { agentQueryOptions } from '../state/agents-queries';
import { isNotFound } from '../state/models';
import { AgentMissing } from './agent-missing';
import { AgentSettingsForm } from './agent-settings-form';
import { ArchiveAgent } from './archive-agent';
import { SetupProgressPanel } from './setup-progress-panel';

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
      <SetupProgressPanel agentId={agentId} />
      <AgentSettingsForm key={agent.data.id} agent={agent.data} />
      <ArchiveAgent agentId={agentId} name={agent.data.settings.name} />
    </div>
  );
}
