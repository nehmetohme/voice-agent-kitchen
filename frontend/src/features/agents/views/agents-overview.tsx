import { Bot } from 'lucide-react';

import { EmptyState } from '../../../shared/ui/empty-state';

/** The Agents home. Until agents can be created (M2), it explains what will appear here. */
export function AgentsOverview() {
  return (
    <EmptyState
      icon={<Bot aria-hidden className="size-8 text-muted" />}
      title="No agents yet"
      description="Agents you create appear here, with their setup progress and where they are live."
    />
  );
}
