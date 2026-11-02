import { Link } from '@tanstack/react-router';
import { CircleCheck, CircleDot, Circle } from 'lucide-react';
import { useId } from 'react';

import { ProgressBar } from '../../../shared/ui/progress-bar';
import type { SetupStep } from '../state/models';
import type { Checklist, StepState } from '../state/setup-checklist';

const ICONS: Record<StepState, typeof Circle> = {
  done: CircleCheck,
  next: CircleDot,
  todo: Circle,
};

const STATE_TEXT: Record<StepState, string> = { done: 'Done', next: 'Next', todo: 'Later' };

/** The steps that have a page yet; the others arrive with their milestones. */
const PAGES: Partial<Record<SetupStep, '/agents/$agentId/provision' | '/agents/$agentId/plan'>> = {
  provision: '/agents/$agentId/provision',
  plan: '/agents/$agentId/plan',
};

function StepLabel({ agentId, step, label }: { agentId: string; step: SetupStep; label: string }) {
  const to = PAGES[step];
  if (to === undefined) return <span className="font-medium">{label}</span>;
  return (
    <Link
      to={to}
      params={{ agentId }}
      className="font-medium underline underline-offset-4"
      activeProps={{ 'aria-current': 'page' }}
    >
      {label}
    </Link>
  );
}

/** The four setup steps and how far the agent is (Zeigarnik). Progress is always visible. */
export function SetupChecklist({ agentId, checklist }: { agentId: string; checklist: Checklist }) {
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
              <StepLabel agentId={agentId} step={step} label={label} />
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
