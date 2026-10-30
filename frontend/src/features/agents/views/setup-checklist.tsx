import { CircleCheck, CircleDot, Circle } from 'lucide-react';
import { useId } from 'react';

import { ProgressBar } from '../../../shared/ui/progress-bar';
import type { Checklist, StepState } from '../state/setup-checklist';

const ICONS: Record<StepState, typeof Circle> = {
  done: CircleCheck,
  next: CircleDot,
  todo: Circle,
};

const STATE_TEXT: Record<StepState, string> = { done: 'Done', next: 'Next', todo: 'Later' };

/** The four setup steps and how far the agent is (Zeigarnik). Progress is always visible. */
export function SetupChecklist({ checklist }: { checklist: Checklist }) {
  const titleId = useId();
  return (
    <section
      aria-labelledby={titleId}
      className="flex flex-col gap-4 rounded-md border border-line bg-surface p-6"
    >
      <h2 id={titleId} className="text-lg font-semibold">
        Setup
      </h2>
      <ProgressBar label="Setup progress" value={checklist.percent} />
      <ol aria-label="Setup steps" className="flex flex-col gap-2">
        {checklist.steps.map(({ step, label, hint, state }) => {
          const Icon = ICONS[state];
          return (
            <li key={step} className="flex items-center gap-3">
              <Icon aria-hidden className={state === 'done' ? 'size-4 text-success' : 'size-4'} />
              <span className="font-medium">{label}</span>
              <span className="text-sm text-muted">
                {STATE_TEXT[state]}: {hint}
              </span>
            </li>
          );
        })}
      </ol>
    </section>
  );
}
