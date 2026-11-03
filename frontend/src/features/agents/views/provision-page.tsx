import { useQuery } from '@tanstack/react-query';
import type { ReactNode } from 'react';

import { LoadFailed, Loading } from '../../../shared/ui/load-states';
import { agentQueryOptions } from '../state/agents-queries';
import { isNotFound } from '../state/models';
import { AgentHeader } from './agent-header';
import { AgentMissing } from './agent-missing';
import { AgentSettingsForm } from './agent-settings-form';

interface ProvisionPageProps {
  agentId: string;
  /** Composed in by the route from another feature: the test call, beside the settings. */
  testCall?: ReactNode;
}

/**
 * The Provision step of one agent as a workspace that fits the screen: the header with the
 * setup stepper, then the settings (autosaved) on the left and the test call on the right.
 * From the `lg` breakpoint the page never scrolls; each column scrolls on its own. Narrower,
 * the columns stack and the page scrolls.
 */
export function ProvisionPage({ agentId, testCall }: ProvisionPageProps) {
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
    <div className="flex flex-col gap-4 lg:h-full">
      <AgentHeader agentId={agentId} />
      <div className="flex flex-col gap-6 lg:min-h-0 lg:flex-1 lg:flex-row">
        <div className="flex min-w-0 flex-col lg:min-h-0 lg:flex-2">
          <AgentSettingsForm key={agent.data.id} agent={agent.data} />
        </div>
        <div className="flex min-w-0 flex-col lg:min-h-0 lg:flex-3">{testCall}</div>
      </div>
    </div>
  );
}
