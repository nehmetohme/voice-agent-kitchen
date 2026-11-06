import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';

import { EmptyState } from '../../../shared/ui/empty-state';
import { LoadFailed, Loading } from '../../../shared/ui/load-states';
import { isNotFound } from '../state/models';
import { planQueryOptions } from '../state/plans-queries';
import { PlanEditor } from './plan-editor';

interface PlanPageProps {
  agentId: string;
  /** Called after each save, e.g. to refresh the setup progress. */
  onSaved?: (() => void) | undefined;
}

/** The Plan step of one agent: its flow, loaded from the server and edited in place. */
export function PlanPage({ agentId, onSaved }: PlanPageProps) {
  const plan = useQuery(planQueryOptions(agentId));
  if (plan.isPending) return <Loading label="Loading the plan…" />;
  if (plan.isError) {
    if (isNotFound(plan.error)) {
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
    return (
      <LoadFailed
        what="the plan"
        onRetry={() => {
          void plan.refetch();
        }}
      />
    );
  }
  return (
    <PlanEditor
      key={agentId}
      target={{ kind: 'agent', agentId }}
      initial={plan.data}
      onSaved={onSaved}
    />
  );
}
