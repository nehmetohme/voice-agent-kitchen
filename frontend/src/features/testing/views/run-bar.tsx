import { Play, Square } from 'lucide-react';
import { useId } from 'react';

import { Button } from '../../../shared/ui/button';
import { ProgressBar } from '../../../shared/ui/progress-bar';
import { startProblemOf } from '../state/models';
import { modelsText, runStatusText } from '../state/run-view';
import { isActive } from '../state/test-runs-queries';
import type { TestPageModel } from '../state/use-test-page';

/** The stale run's notice: why the Test step isn't done although the run passed. */
function Stale() {
  return (
    <p className="rounded-md border border-warning px-3 py-2 text-sm">
      This run tested an earlier version of the agent. Run the tests again to finish the Test step.
    </p>
  );
}

/**
 * The run's header: its outcome first (a polite status), its progress while it runs, and the
 * one primary action, "Run tests", or "Cancel run" while a run is under way.
 */
export function RunBar({ page }: { page: TestPageModel }) {
  const titleId = useId();
  const { run, stale } = page.latest;
  const active = run !== null && isActive(run.status);
  const models = modelsText(run);
  const startError = page.start.isError ? startProblemOf(page.start.error) : null;
  return (
    <section
      aria-labelledby={titleId}
      className="flex flex-col gap-2 rounded-md border border-line p-4"
    >
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex min-w-0 flex-col">
          <h2 id={titleId} className="text-lg font-semibold">
            Test run
          </h2>
          <output aria-label="Test run status" className="text-sm">
            {page.latest.loading ? 'Loading the latest run…' : runStatusText(run)}
          </output>
          {models !== null && <p className="text-sm text-muted">{models}</p>}
        </div>
        <div data-region="actions" className="flex gap-2">
          {active ? (
            <Button
              aria-busy={page.cancel.isPending}
              disabled={page.cancel.isPending}
              onClick={() => {
                page.cancel.mutate(run.id);
              }}
            >
              <Square aria-hidden className="size-4" />
              Cancel run
            </Button>
          ) : (
            <Button
              variant="primary"
              aria-busy={page.start.isPending}
              disabled={page.start.isPending}
              onClick={() => {
                page.start.mutate();
              }}
            >
              <Play aria-hidden className="size-4" />
              Run tests
            </Button>
          )}
        </div>
      </div>
      {active && <ProgressBar label="Test run progress" value={run.progress.percent} />}
      {stale && !active && <Stale />}
      {startError !== null && (
        <p role="alert" className="text-sm text-danger">
          {startError}
        </p>
      )}
    </section>
  );
}
