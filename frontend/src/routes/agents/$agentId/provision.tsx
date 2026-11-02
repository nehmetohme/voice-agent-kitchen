import { createFileRoute } from '@tanstack/react-router';

import { ProvisionPage } from '../../../features/agents';
import { LatencyPanel, TestCallPanel } from '../../../features/testing';

export const Route = createFileRoute('/agents/$agentId/provision')({
  staticData: { title: 'Provision' },
  component: ProvisionRoute,
});

/** Until the Test step exists (M5), the test call and its latency sit on this page. */
function ProvisionRoute() {
  const { agentId } = Route.useParams();
  return (
    <ProvisionPage
      agentId={agentId}
      testCall={
        <>
          <TestCallPanel agentId={agentId} />
          <LatencyPanel />
        </>
      }
    />
  );
}
