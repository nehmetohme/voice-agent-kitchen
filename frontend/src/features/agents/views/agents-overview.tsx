import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Bot, Plus } from 'lucide-react';

import { buttonLook } from '../../../shared/ui/button';
import { EmptyState } from '../../../shared/ui/empty-state';
import { agentsQueryOptions } from '../state/agents-queries';
import { AgentList } from './agent-list';
import { LoadFailed, Loading } from './load-states';

/** The one primary action of the Agents screen (Von Restorff). */
function CreateAgentLink() {
  return (
    <Link to="/agents/new" {...buttonLook('primary')}>
      <Plus aria-hidden className="size-4" />
      Create agent
    </Link>
  );
}

/** The Agents home: every active agent with its setup progress, or how to create the first. */
export function AgentsOverview() {
  const agents = useQuery(agentsQueryOptions());
  if (agents.isPending) return <Loading label="Loading agents…" />;
  if (agents.isError) {
    return (
      <LoadFailed
        what="your agents"
        onRetry={() => {
          void agents.refetch();
        }}
      />
    );
  }
  if (agents.data.length === 0) {
    return (
      <EmptyState
        icon={<Bot aria-hidden className="size-8 text-muted" />}
        title="No agents yet"
        description="Create your first agent: give it a name, a purpose and a voice. Everything else starts from a sensible default."
        action={<CreateAgentLink />}
      />
    );
  }
  return (
    <div className="flex max-w-readable flex-col gap-6">
      <div className="flex items-center justify-between gap-4">
        <p className="text-muted">Most recently edited first.</p>
        <CreateAgentLink />
      </div>
      <AgentList agents={agents.data} />
    </div>
  );
}
