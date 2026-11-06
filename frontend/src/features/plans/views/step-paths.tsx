import { ArrowRight, X } from 'lucide-react';

import { Button } from '../../../shared/ui/button';
import type { Path, Problem, Step } from '../state/models';

interface StepPathsProps {
  paths: readonly Path[];
  steps: readonly Step[];
  /** Problems on these paths: each is shown on the path it is about. */
  problems: readonly Problem[];
  /** Left out when the flow is only shown. */
  onRemove?: ((pathId: string) => void) | undefined;
}

/**
 * The paths out of a step, in words (when → where): the canvas draws them as lines, and this
 * list is what keyboard and screen-reader users act on.
 */
export function StepPaths({ paths, steps, problems, onRemove }: StepPathsProps) {
  if (paths.length === 0) return null;
  const labelOf = (id: string) => steps.find((step) => step.id === id)?.label ?? 'a missing step';
  return (
    <ul aria-label="Paths" className="flex flex-col gap-2">
      {paths.map((path) => {
        const target = labelOf(path.to);
        const own = problems.filter((problem) => problem.pathId === path.id);
        return (
          <li key={path.id} className="flex flex-col gap-1 rounded-md border border-line p-2">
            <div className="flex items-center gap-2">
              <p className="flex flex-1 flex-wrap items-center gap-1 text-sm">
                <span className={path.condition === '' ? 'text-muted italic' : undefined}>
                  {path.condition === '' ? 'No condition yet' : path.condition}
                </span>
                <ArrowRight aria-hidden className="size-4 text-muted" />
                <span className="font-medium">{target}</span>
              </p>
              {onRemove !== undefined && (
                <Button
                  variant="ghost"
                  aria-label={`Remove path to ${target}`}
                  onClick={() => {
                    onRemove(path.id);
                  }}
                >
                  <X aria-hidden className="size-4" />
                </Button>
              )}
            </div>
            {own.map((problem) => (
              <p key={problem.message} className="text-sm text-danger">
                {problem.message}
              </p>
            ))}
          </li>
        );
      })}
    </ul>
  );
}
