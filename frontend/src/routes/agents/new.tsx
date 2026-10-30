import { createFileRoute } from '@tanstack/react-router';

import { CreateAgentPage } from '../../features/agents';

export const Route = createFileRoute('/agents/new')({
  staticData: { title: 'Create agent' },
  component: CreateAgentPage,
});
