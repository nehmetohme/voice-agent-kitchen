import { CircleAlert, CircleCheck, CircleDashed, CircleX, LoaderCircle } from 'lucide-react';
import { useState } from 'react';

import { cn } from '../../../shared/lib/cn';
import { Button } from '../../../shared/ui/button';
import { ConfirmDialog } from '../../../shared/ui/confirm-dialog';
import type { Scenario } from '../state/models';
import { STANDING_LABEL, type Standing } from '../state/run-view';
import { ScenarioDialog } from './scenario-dialog';

const ICON: Record<Standing, typeof CircleCheck> = {
  passed: CircleCheck,
  failed: CircleX,
  error: CircleAlert,
  running: LoaderCircle,
  pending: CircleDashed,
  skipped: CircleDashed,
  unrun: CircleDashed,
};

const TONE: Partial<Record<Standing, string>> = {
  passed: 'text-success',
  failed: 'text-danger',
  error: 'text-danger',
};

interface ScenarioRowProps {
  scenario: Scenario;
  scenarios: readonly Scenario[];
  standing: Standing;
  selected: boolean;
  onSelect: () => void;
  onSave: (scenario: Scenario) => void;
  onDelete: () => void;
}

/**
 * One scenario: a button that shows its result (its name, "Required", and how it did in the
 * latest run), then its own Edit and Delete (FITTS-5). The status is in words, not colour only;
 * only its icon is coloured, so the words keep their contrast on the selected row.
 */
export function ScenarioRow({
  scenario,
  scenarios,
  standing,
  selected,
  onSelect,
  onSave,
  onDelete,
}: ScenarioRowProps) {
  const Icon = ICON[standing];
  // Controlled, so the dialog closes on Delete even when the set refuses the change: the list
  // then says why (a set keeps one required scenario).
  const [confirming, setConfirming] = useState(false);
  return (
    <li className="flex items-center gap-2">
      <Button
        variant="ghost"
        aria-current={selected ? 'true' : undefined}
        className="min-w-0 flex-1 justify-start aria-[current=true]:bg-selected"
        onClick={onSelect}
      >
        <span className="min-w-0 flex-1 truncate text-left">{scenario.name}</span>
        {scenario.required && (
          <span className="rounded-full border border-line px-2 text-sm text-muted">Required</span>
        )}
        <span className="flex items-center gap-1 text-sm">
          <Icon
            aria-hidden
            className={cn(
              'size-4',
              TONE[standing] ?? 'text-muted',
              standing === 'running' && 'animate-spin',
            )}
          />
          {STANDING_LABEL[standing]}
        </span>
      </Button>
      <div data-region="actions" className="flex gap-2">
        <ScenarioDialog scenario={scenario} scenarios={scenarios} onSave={onSave} />
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
            onDelete();
          }}
        />
      </div>
    </li>
  );
}
