import { CircleAlert, CircleCheck } from 'lucide-react';
import { useId } from 'react';

import { Button } from '../../../shared/ui/button';
import { SaveStatus } from '../../../shared/ui/save-status';
import type { PlanTarget, PlanView } from '../state/models';
import { usePlanEditor } from '../state/use-plan-editor';
import { AddStepDialog } from './add-step-dialog';
import { LockedOpening } from './locked-opening';
import { PlanCanvas } from './plan-canvas';

interface PlanEditorProps {
  target: PlanTarget;
  initial: PlanView;
  /** Called after each save, e.g. to refresh the setup progress. */
  onSaved?: (() => void) | undefined;
  /** Shown but not changed: a built-in purpose's flow. */
  readOnly?: boolean | undefined;
}

/** One line on how the plan stands: valid, or how many problems are left. */
function Summary({ view }: { view: PlanView }) {
  if (view.valid) {
    return (
      <p className="flex items-center gap-2 text-sm text-muted">
        <CircleCheck aria-hidden className="size-4 text-success" />
        The plan follows every rule.
      </p>
    );
  }
  const count = view.problems.length;
  return (
    <p className="flex items-center gap-2 text-sm text-danger">
      <CircleAlert aria-hidden className="size-4" />
      {count === 1 ? '1 problem to fix' : `${count} problems to fix`}, shown where they are.
    </p>
  );
}

/**
 * The flow editor: the steps and their paths, autosaved. Until an agent's plan is saved the
 * starting flow is shown, and "Use this plan" (or any edit) saves it. A purpose's flow is
 * edited the same way; read-only, it is only shown.
 */
export function PlanEditor({ target, initial, onSaved, readOnly = false }: PlanEditorProps) {
  const editor = usePlanEditor(target, initial, {
    readOnly,
    ...(onSaved === undefined ? {} : { onSaved }),
  });
  const { view, status } = editor;
  const titleId = useId();
  const planProblems = view.problems.filter(
    (problem) => problem.stepId === null && problem.pathId === null,
  );
  return (
    <section aria-labelledby={titleId} className="flex flex-col gap-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="flex flex-col gap-1">
          <h2 id={titleId} className="text-lg font-semibold">
            Flow
          </h2>
          {view.saved ? (
            <Summary view={view} />
          ) : (
            <p className="text-sm text-muted">
              This is the starting plan for this purpose. Use it as it is, or change it.
            </p>
          )}
        </div>
        {!readOnly && (
          <div data-region="actions" className="flex flex-wrap items-center gap-2">
            <SaveStatus status={status} onRetry={editor.retry} />
            <AddStepDialog onAdd={editor.addStep} />
            {!view.saved && (
              <Button
                variant="primary"
                aria-busy={status === 'saving'}
                disabled={status === 'saving'}
                onClick={editor.keepTemplate}
              >
                Use this plan
              </Button>
            )}
          </div>
        )}
      </div>
      {view.prependedTexts.length > 0 && (
        <div className="flex max-w-readable flex-col gap-2">
          <p className="text-sm text-muted">
            Every agent&apos;s opening starts with its rule pack&apos;s locked texts, then this
            flow&apos;s greeting. In the US:
          </p>
          <LockedOpening locked={view.prependedTexts} />
        </div>
      )}
      {planProblems.length > 0 && (
        <ul aria-label="Plan problems" className="flex flex-col gap-1">
          {planProblems.map((problem) => (
            <li key={problem.message} className="flex gap-2 text-danger">
              <CircleAlert aria-hidden className="mt-1 size-4 shrink-0" />
              {problem.message}
            </li>
          ))}
        </ul>
      )}
      <PlanCanvas editor={editor} />
    </section>
  );
}
