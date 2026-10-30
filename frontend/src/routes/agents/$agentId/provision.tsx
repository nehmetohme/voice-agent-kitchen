import { createFileRoute } from '@tanstack/react-router';

import { ProvisionPage } from '../../../features/agents';

export const Route = createFileRoute('/agents/$agentId/provision')({
  staticData: { title: 'Provision' },
  component: ProvisionRoute,
});

function ProvisionRoute() {
  const { agentId } = Route.useParams();
  return <ProvisionPage agentId={agentId} />;
}
