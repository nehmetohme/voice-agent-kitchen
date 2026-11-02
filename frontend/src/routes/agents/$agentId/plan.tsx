import { createFileRoute } from '@tanstack/react-router';

import { SetupProgressPanel, useRefreshSetupProgress } from '../../../features/agents';
import { PlanPage } from '../../../features/plans';

export const Route = createFileRoute('/agents/$agentId/plan')({
  staticData: { title: 'Plan' },
  component: PlanRoute,
});

/** The Plan step: setup progress first (Zeigarnik), then the flow; a save refreshes progress. */
function PlanRoute() {
  const { agentId } = Route.useParams();
  const refreshProgress = useRefreshSetupProgress(agentId);
  return (
    <div className="flex flex-col gap-8">
      <div className="max-w-readable">
        <SetupProgressPanel agentId={agentId} />
      </div>
      <PlanPage agentId={agentId} onSaved={refreshProgress} />
    </div>
  );
}
