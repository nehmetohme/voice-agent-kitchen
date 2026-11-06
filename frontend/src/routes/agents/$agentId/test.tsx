import { createFileRoute } from '@tanstack/react-router';

import { AgentHeader, useRefreshSetupProgress } from '../../../features/agents';
import { TestPage } from '../../../features/testing';

export const Route = createFileRoute('/agents/$agentId/test')({
  staticData: { title: 'Test' },
  component: TestRoute,
});

/**
 * The Test step: the agent's header and setup stepper first, then its scenarios and test runs.
 * A saved scenario set or a finished run refreshes progress (the Test step is 75 %).
 */
function TestRoute() {
  const { agentId } = Route.useParams();
  const refreshProgress = useRefreshSetupProgress(agentId);
  return (
    <div className="flex flex-col gap-4 lg:h-full">
      <AgentHeader agentId={agentId} />
      <TestPage agentId={agentId} onChange={refreshProgress} />
    </div>
  );
}
