import { useId } from 'react';

import { standingOf } from '../state/run-view';
import type { TestPageModel } from '../state/use-test-page';
import { ScenarioDialog } from './scenario-dialog';
import { ScenarioRow } from './scenario-row';

/**
 * The agent's scenarios, each with how it did in the latest run; picking one shows its result,
 * and its Edit and Delete, beside the list. Until scenarios are saved, these are the use case's
 * defaults. The list scrolls on its own; "Add scenario" stays under it.
 */
export function ScenarioList({ page }: { page: TestPageModel }) {
  const titleId = useId();
  const scenarios = page.scenarios.data?.scenarios ?? [];
  const saved = page.scenarios.data?.saved ?? false;
  return (
    <section
      aria-labelledby={titleId}
      aria-busy={page.saving}
      className="flex flex-col gap-3 rounded-md border border-line p-4 lg:min-h-0 lg:flex-1"
    >
      <div>
        <h2 id={titleId} className="text-lg font-semibold">
          Scenarios
        </h2>
        <p className="text-sm text-muted">
          {saved
            ? 'Simulated callers play each one; a rubric scores it.'
            : 'The defaults for this purpose. Edit one to make the set your own.'}
        </p>
      </div>
      <ul
        aria-label="Scenarios"
        className="flex flex-col gap-2 lg:min-h-0 lg:flex-1 lg:overflow-y-auto"
      >
        {scenarios.map((scenario) => (
          <ScenarioRow
            key={scenario.id}
            scenario={scenario}
            standing={standingOf(page.latest.run, scenario.id)}
            selected={page.selected?.id === scenario.id}
            onSelect={() => {
              page.select(scenario.id);
            }}
          />
        ))}
      </ul>
      {page.setProblem !== null && (
        <p role="alert" className="text-sm text-danger">
          {page.setProblem}
        </p>
      )}
      <div data-region="actions" className="flex">
        <ScenarioDialog scenarios={scenarios} onSave={page.saveScenario} />
      </div>
    </section>
  );
}
