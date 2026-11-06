import { useState } from 'react';

import { Button } from '../../../shared/ui/button';
import { ConfirmDialog } from '../../../shared/ui/confirm-dialog';
import type { Scenario } from '../state/models';
import type { TestPageModel } from '../state/use-test-page';
import { ScenarioDialog } from './scenario-dialog';

/**
 * The selected scenario's own actions, next to its name: Edit (a dialog) and Delete (after a
 * confirmation). The confirmation closes on Delete even when the set refuses the change (it
 * keeps one required scenario): the list then says why.
 */
export function ScenarioActions({ scenario, page }: { scenario: Scenario; page: TestPageModel }) {
  const [confirming, setConfirming] = useState(false);
  const scenarios = page.scenarios.data?.scenarios ?? [];
  return (
    <div data-region="actions" className="flex shrink-0 gap-2">
      <ScenarioDialog scenario={scenario} scenarios={scenarios} onSave={page.saveScenario} />
      <Button
        variant="danger"
        onClick={() => {
          setConfirming(true);
        }}
      >
        Delete
      </Button>
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title={`Delete ${scenario.name}?`}
        description="It won't be part of the next test run."
        confirmLabel="Delete"
        onConfirm={() => {
          setConfirming(false);
          page.deleteScenario(scenario.id);
        }}
      />
    </div>
  );
}
