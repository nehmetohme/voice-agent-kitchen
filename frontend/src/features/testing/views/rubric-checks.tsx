import { CircleAlert, CircleCheck, CircleDashed, CircleX } from 'lucide-react';

import { cn } from '../../../shared/lib/cn';
import type { RubricCheck, RubricItem } from '../state/models';
import { checkText } from '../state/run-view';

/** A check's outcome: passed, failed, no answer, or (before a run) not scored yet. */
function Outcome({ check }: { check: RubricCheck | null }) {
  if (check === null) {
    return (
      <span className="flex items-center gap-1 text-muted">
        <CircleDashed aria-hidden className="size-4" />
        Not scored
      </span>
    );
  }
  if (check.probability === null) {
    return (
      <span className="flex items-center gap-1 text-danger">
        <CircleAlert aria-hidden className="size-4" />
        No answer
      </span>
    );
  }
  const Icon = check.passed ? CircleCheck : CircleX;
  return (
    <span className={cn('flex items-center gap-1', check.passed ? 'text-success' : 'text-danger')}>
      <Icon aria-hidden className="size-4" />
      {check.passed ? 'Passed' : 'Failed'}
    </span>
  );
}

interface RubricChecksProps {
  /** The scenario's rubric, shown until it has checks. */
  rubric: readonly RubricItem[];
  /** The latest run's checks, in rubric order; empty before it was scored. */
  checks: readonly RubricCheck[];
}

/**
 * The rubric, one line per question: whether it passed (in words and with an icon), the
 * question, and the expected answer next to the model's.
 */
export function RubricChecks({ rubric, checks }: RubricChecksProps) {
  const rows =
    checks.length > 0
      ? checks.map((check) => ({ question: check.question, check }))
      : rubric.map((item) => ({ question: item.question, check: null }));
  return (
    <ul aria-label="Rubric checks" className="flex flex-col gap-2">
      {rows.map(({ question, check }) => (
        <li
          key={question}
          className="flex flex-col gap-1 rounded-md border border-line p-3 text-sm"
        >
          <span className="flex flex-wrap items-center gap-x-3 gap-y-1">
            <Outcome check={check} />
            <span className="font-medium">{question}</span>
          </span>
          {check !== null && <span className="text-muted">{checkText(check)}</span>}
        </li>
      ))}
    </ul>
  );
}
