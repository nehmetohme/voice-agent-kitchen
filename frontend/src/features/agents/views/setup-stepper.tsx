import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Circle, CircleCheck, CircleDot } from 'lucide-react';

import { cn } from '../../../shared/lib/cn';
import { Button } from '../../../shared/ui/button';
import { ProgressBar } from '../../../shared/ui/progress-bar';
import { agentProgressQueryOptions } from '../state/agents-queries';
import type { SetupStep } from '../state/models';
import { toChecklist, type Checklist, type StepState } from '../state/setup-checklist';

const ICONS: Record<StepState, typeof Circle> = {
  done: CircleCheck,
  next: CircleDot,
  todo: Circle,
};

const STATE_TEXT: Record<StepState, string> = { done: 'Done', next: 'Next', todo: 'To do' };

/** The steps that have a page yet. Test arrives in M5 and Publish in M6. */
const PAGES: Partial<Record<SetupStep, '/agents/$agentId/provision' | '/agents/$agentId/plan'>> = {
  provision: '/agents/$agentId/provision',
  plan: '/agents/$agentId/plan',
};

const STEP = 'flex min-h-target min-w-target items-center gap-2 rounded-md px-2 text-sm';

const LINK = cn(
  STEP,
  'font-medium transition-colors hover:bg-hover',
  'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus',
  'aria-[current=page]:bg-selected',
);

/** The visible tag after a step: "Next" stands out, a step without a page yet says "Later". */
function Tag({ state, available }: { state: StepState; available: boolean }) {
  if (!available)
    return <span className="rounded-full border border-line px-2 text-sm text-muted">Later</span>;
  if (state === 'next') {
    return (
      <span className="rounded-full border border-ink px-2 text-sm font-medium text-ink">Next</span>
    );
  }
  return <span className="sr-only">{STATE_TEXT[state]}</span>;
}

function Steps({ agentId, checklist }: { agentId: string; checklist: Checklist }) {
  return (
    <ol aria-label="Setup steps" className="flex flex-wrap items-center gap-x-1 gap-y-2">
      {checklist.steps.map(({ step, label, hint, state }, index) => {
        const Icon = ICONS[state];
        const to = PAGES[step];
        const icon = (
          <Icon aria-hidden className={cn('size-4', state === 'done' && 'text-success')} />
        );
        const doneBefore = checklist.steps[index - 1]?.state === 'done';
        return (
          <li key={step} className="flex items-center gap-1">
            {index > 0 && (
              <span aria-hidden className={cn('h-px w-4', doneBefore ? 'bg-success' : 'bg-line')} />
            )}
            {to === undefined ? (
              <span className={cn(STEP, 'text-muted')}>
                {icon}
                {label}
              </span>
            ) : (
              <Link
                to={to}
                params={{ agentId }}
                className={LINK}
                activeProps={{ 'aria-current': 'page' }}
              >
                {icon}
                {label}
              </Link>
            )}
            <Tag state={state} available={to !== undefined} />
            <span className="sr-only">: {hint}</span>
          </li>
        );
      })}
    </ol>
  );
}

/**
 * The four setup steps as the agent's navigation, with how far it is (Zeigarnik): done steps
 * are ticked, the next one is marked, and steps not built yet say "Later" instead of looking
 * broken. Progress reaches 50 % with a valid plan, 75 % with Test and 100 % with Publish.
 */
export function SetupStepper({ agentId }: { agentId: string }) {
  const progress = useQuery(agentProgressQueryOptions(agentId));
  if (progress.isPending) {
    return (
      <p aria-busy="true" className="text-sm text-muted">
        Loading setup progress…
      </p>
    );
  }
  if (progress.isError) {
    return (
      <div role="alert" className="flex items-center gap-2 text-sm">
        <p>Couldn't load setup progress.</p>
        <Button
          variant="ghost"
          onClick={() => {
            void progress.refetch();
          }}
        >
          Try again
        </Button>
      </div>
    );
  }
  const checklist = toChecklist(progress.data);
  return (
    <nav aria-label="Setup" className="flex flex-wrap items-center gap-x-4 gap-y-2">
      <Steps agentId={agentId} checklist={checklist} />
      <ProgressBar compact label="Setup progress" value={checklist.percent} />
    </nav>
  );
}
