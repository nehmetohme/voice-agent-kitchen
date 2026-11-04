import { useQuery } from '@tanstack/react-query';

import { agentQueryOptions } from '../state/agents-queries';
import { AgentActions } from './agent-actions';
import { SetupStepper } from './setup-stepper';

/**
 * One row above every step of an agent: its name, the setup stepper that moves between the
 * steps (always in view: Zeigarnik), and its rare actions. Shared by the Provision and Plan
 * routes. While the agent loads it says so; when it is missing, the page below says what to do.
 */
export function AgentHeader({ agentId }: { agentId: string }) {
  const agent = useQuery(agentQueryOptions(agentId));
  if (agent.isPending) {
    return (
      <p aria-busy="true" className="flex min-h-target items-center text-sm text-muted">
        Loading the agent…
      </p>
    );
  }
  if (agent.isError || agent.data.archived) return null;
  const { name } = agent.data.settings;
  return (
    <div className="flex flex-wrap items-center gap-x-6 gap-y-2 lg:flex-nowrap">
      <h2 className="min-w-0 flex-1 truncate text-xl font-semibold" title={name}>
        {name}
      </h2>
      {/* Narrower than `lg`, the stepper takes its own row under the name and the menu. */}
      <div className="max-lg:order-last max-lg:basis-full">
        <SetupStepper agentId={agentId} />
      </div>
      <AgentActions agentId={agentId} name={name} />
    </div>
  );
}
