import { Link } from '@tanstack/react-router';

import { EmptyState } from '../../../shared/ui/empty-state';
import { LoadFailed, Loading } from '../../../shared/ui/load-states';
import { isNotFound } from '../state/models';
import { useTestPage } from '../state/use-test-page';
import { RunBar } from './run-bar';
import { ScenarioList } from './scenario-list';
import { ScenarioResult } from './scenario-result';

interface TestPageProps {
  agentId: string;
  /** Called when the draft or the Test step may have changed, e.g. to refresh progress. */
  onChange?: (() => void) | undefined;
}

/**
 * The Test step of one agent as a workspace that fits the screen: the run first (its outcome,
 * progress and "Run tests"), then the scenarios on the left and the selected one's result on
 * the right. From `lg` the page never scrolls; each column scrolls on its own.
 */
export function TestPage({ agentId, onChange }: TestPageProps) {
  const page = useTestPage(agentId, onChange);
  if (page.scenarios.isPending) return <Loading label="Loading the scenarios…" />;
  if (page.scenarios.isError) {
    if (isNotFound(page.scenarios.error)) {
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
        what="the scenarios"
        onRetry={() => {
          void page.scenarios.refetch();
        }}
      />
    );
  }
  return (
    <div className="flex flex-col gap-4 lg:min-h-0 lg:flex-1">
      <RunBar page={page} />
      <div className="flex flex-col gap-6 lg:min-h-0 lg:flex-1 lg:flex-row">
        <div className="flex min-w-0 flex-col lg:min-h-0 lg:flex-2">
          <ScenarioList page={page} />
        </div>
        <div className="flex min-w-0 flex-col lg:min-h-0 lg:flex-3">
          <ScenarioResult page={page} />
        </div>
      </div>
    </div>
  );
}
