import { Link } from '@tanstack/react-router';

import { EmptyState } from '../../../shared/ui/empty-state';

/** An agent that doesn't exist (or no longer does), with a way back. */
export function AgentMissing() {
  return (
    <EmptyState
      title="This agent doesn't exist"
      description="It may have been archived, or the address is wrong."
      action={
        <Link to="/agents" className="font-medium underline underline-offset-4">
          Go to Agents
        </Link>
      }
    />
  );
}
