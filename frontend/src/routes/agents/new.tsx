import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';

import { CreateAgentPage } from '../../features/agents';

/** `?purpose=`: a purpose to start with, as the pickers name it (from a purpose's page). */
const search = z.object({ purpose: z.string().max(64).optional().catch(undefined) });

export const Route = createFileRoute('/agents/new')({
  staticData: { title: 'Create agent' },
  validateSearch: search,
  component: CreateAgentRoute,
});

function CreateAgentRoute() {
  const { purpose } = Route.useSearch();
  return <CreateAgentPage initialPurpose={purpose} />;
}
