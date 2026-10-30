import { createFileRoute } from '@tanstack/react-router';

import { AgentsOverview } from '../../features/agents';

export const Route = createFileRoute('/agents/')({
  staticData: { title: 'Agents' },
  component: AgentsOverview,
});
