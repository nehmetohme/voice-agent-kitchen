import { createFileRoute } from '@tanstack/react-router';

import { AgentHeader, useRefreshSetupProgress } from '../../../features/agents';
import { PlanPage } from '../../../features/plans';

export const Route = createFileRoute('/agents/$agentId/plan')({
  staticData: { title: 'Plan' },
  component: PlanRoute,
});

/** The Plan step: the agent's header and setup stepper first, then the flow; a save refreshes progress. */
function PlanRoute() {
  const { agentId } = Route.useParams();
  const refreshProgress = useRefreshSetupProgress(agentId);
  return (
    <div className="flex flex-col gap-6">
      <AgentHeader agentId={agentId} />
      <PlanPage agentId={agentId} onSaved={refreshProgress} />
    </div>
  );
}
