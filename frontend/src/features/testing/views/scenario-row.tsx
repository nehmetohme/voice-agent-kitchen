import { CircleAlert, CircleCheck, CircleDashed, CircleX, LoaderCircle } from 'lucide-react';

import { cn } from '../../../shared/lib/cn';
import { Button } from '../../../shared/ui/button';
import type { Scenario } from '../state/models';
import { STANDING_LABEL, type Standing } from '../state/run-view';

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
  standing: Standing;
  selected: boolean;
  onSelect: () => void;
}

/**
 * One scenario: a button that shows it beside the list, with its name on the first line and
 * "Required" and how it did in the latest run on the second. The status is in words, not colour
 * only; only its icon is coloured, so the words keep their contrast on the selected row.
 */
export function ScenarioRow({ scenario, standing, selected, onSelect }: ScenarioRowProps) {
  const Icon = ICON[standing];
  return (
    <li>
      <Button
        variant="ghost"
        aria-current={selected ? 'true' : undefined}
        className="w-full justify-start py-2 aria-[current=true]:bg-selected"
        onClick={onSelect}
      >
        <span className="flex min-w-0 flex-col items-start text-left">
          <span className="w-full truncate">{scenario.name}</span>
          <span className="flex items-center gap-1 font-normal">
            {scenario.required && <span>Required ·</span>}
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
        </span>
      </Button>
    </li>
  );
}
