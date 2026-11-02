import { CircleAlert } from 'lucide-react';

import { cn } from '../../../shared/lib/cn';
import { ConfirmDialog } from '../../../shared/ui/confirm-dialog';
import type { Step } from '../state/models';
import { splitOpening } from '../state/plan-edits';
import type { PlanEditor } from '../state/use-plan-editor';
import { AddPathDialog } from './add-path-dialog';
import { EditStepDialog } from './edit-step-dialog';
import { LockedOpening } from './locked-opening';
import { STEP_KINDS } from './step-kinds';
import { StepPaths } from './step-paths';

/**
 * One step on the canvas: a named group (its label) with what the agent says, its paths, its
 * problems, and its own actions on its own toolbar (FITTS-5).
 */
export function StepCard({ step, editor }: { step: Step; editor: PlanEditor }) {
  const { draft, view } = editor;
  const { label: kind, Icon } = STEP_KINDS[step.kind];
  const paths = draft.paths.filter((path) => path.from === step.id);
  const pathIds = new Set(paths.map((path) => path.id));
  const problems = view.problems.filter((problem) => problem.stepId === step.id);
  const pathProblems = view.problems.filter(
    (problem) => problem.pathId !== null && pathIds.has(problem.pathId),
  );
  const isStart = step.kind === 'start';
  const opening = splitOpening(step.text, view.lockedTexts);
  const text = isStart && opening.intact ? opening.greeting : step.text;
  return (
    <fieldset
      aria-label={step.label}
      data-step-id={step.id}
      className={cn(
        'relative flex w-step min-w-0 flex-col gap-3 rounded-md border bg-canvas p-4 shadow-sm',
        problems.length > 0 ? 'border-danger' : 'border-line',
      )}
    >
      <p className="flex items-center gap-2 text-sm text-muted">
        <Icon aria-hidden className="size-4" />
        {kind}
      </p>
      <h3 className="font-semibold">{step.label}</h3>
      {isStart && opening.intact && <LockedOpening locked={view.lockedTexts} />}
      {text !== '' && <p className="text-sm whitespace-pre-line">{text}</p>}
      <StepPaths
        paths={paths}
        steps={draft.steps}
        problems={pathProblems}
        onRemove={editor.deletePath}
      />
      {problems.length > 0 && (
        <ul aria-label="Problems" className="flex flex-col gap-1">
          {problems.map((problem) => (
            <li key={problem.message} className="flex gap-2 text-sm text-danger">
              <CircleAlert aria-hidden className="mt-1 size-4 shrink-0" />
              {problem.message}
            </li>
          ))}
        </ul>
      )}
      <div data-region="actions" className="flex flex-wrap gap-2">
        <EditStepDialog
          step={step}
          locked={view.lockedTexts}
          onSave={(changes) => {
            editor.updateStep(step.id, changes);
          }}
        />
        <AddPathDialog from={step} steps={draft.steps} onAdd={editor.addPath} />
        {!isStart && (
          <ConfirmDialog
            triggerLabel="Delete step"
            title={`Delete ${step.label}?`}
            description="The step and every path to and from it go."
            confirmLabel="Delete"
            onConfirm={() => {
              editor.deleteStep(step.id);
            }}
          />
        )}
      </div>
    </fieldset>
  );
}
