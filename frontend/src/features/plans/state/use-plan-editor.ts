import { useCallback, useEffect, useState } from 'react';

import type { SaveState } from '../../../shared/ui/save-status';
import { isSendable, type Path, type PlanDraft, type PlanView, type Step } from '../api/plans-api';
import { addPath, addStep, deletePath, deleteStep, sameDraft, updateStep } from './plan-edits';
import { useSavePlan } from './plans-queries';

/** Quiet time after the last edit before saving: well inside the 1 s budget (PARK-1). */
const AUTOSAVE_DELAY_MS = 500;

export interface PlanEditor {
  /** What the canvas shows: the operator's edits, saved or not. */
  draft: PlanDraft;
  /** The server's last answer: whether a plan is saved, its rules and its problems. */
  view: PlanView;
  status: SaveState;
  /** Saves the template as it is (the "Use this plan" action). */
  keepTemplate: () => void;
  /** Saves again after a failure. */
  retry: () => void;
  addStep: (step: Pick<Step, 'label' | 'kind'>) => void;
  updateStep: (id: string, changes: Partial<Pick<Step, 'label' | 'text'>>) => void;
  deleteStep: (id: string) => void;
  addPath: (path: Omit<Path, 'id'>) => void;
  deletePath: (id: string) => void;
}

function draftOf(view: PlanView): PlanDraft {
  return { steps: view.steps, paths: view.paths };
}

function stateOf(input: {
  invalid: boolean;
  saving: boolean;
  failed: boolean;
  dirty: boolean;
}): SaveState {
  if (input.invalid) return 'invalid';
  if (input.saving) return 'saving';
  if (input.failed) return 'failed';
  return input.dirty ? 'unsaved' : 'saved';
}

/**
 * Holds the editable plan and saves each change `delayMs` after the last edit. Until a plan is
 * saved the template is shown and nothing is sent: the first edit, or "Use this plan", saves.
 */
export function usePlanEditor(
  agentId: string,
  initial: PlanView,
  { onSaved, delayMs = AUTOSAVE_DELAY_MS }: { onSaved?: () => void; delayMs?: number } = {},
): PlanEditor {
  const { mutate, isPending } = useSavePlan(agentId, onSaved);
  const [view, setView] = useState(initial);
  const [draft, setDraft] = useState(() => draftOf(initial));
  // What the server stores: `null` while only the template exists.
  const [stored, setStored] = useState<PlanDraft | null>(() =>
    initial.saved ? draftOf(initial) : null,
  );
  const [edited, setEdited] = useState(false);
  const [failedDraft, setFailedDraft] = useState<PlanDraft | null>(null);

  const dirty = stored === null ? edited : !sameDraft(draft, stored);
  const invalid = !isSendable(draft);
  const failed = failedDraft === draft;

  const save = useCallback(
    (next: PlanDraft) => {
      mutate(next, {
        onSuccess: (answer) => {
          setView(answer);
          setStored(next);
        },
        onError: () => {
          setFailedDraft(next);
        },
      });
    },
    [mutate],
  );

  useEffect(() => {
    if (!dirty || invalid || isPending || failed) return undefined;
    const timer = setTimeout(() => {
      save(draft);
    }, delayMs);
    return () => {
      clearTimeout(timer);
    };
  }, [delayMs, dirty, draft, failed, invalid, isPending, save]);

  const edit = useCallback((change: (current: PlanDraft) => PlanDraft) => {
    setEdited(true);
    setDraft(change);
  }, []);

  return {
    draft,
    view,
    status: stateOf({ invalid, saving: isPending, failed, dirty: dirty || stored === null }),
    keepTemplate: () => {
      save(draft);
    },
    retry: () => {
      setFailedDraft(null);
    },
    addStep: (step) => {
      edit((current) => addStep(current, step).draft);
    },
    updateStep: (id, changes) => {
      edit((current) => updateStep(current, id, changes));
    },
    deleteStep: (id) => {
      edit((current) => deleteStep(current, id));
    },
    addPath: (path) => {
      edit((current) => addPath(current, path));
    },
    deletePath: (id) => {
      edit((current) => deletePath(current, id));
    },
  };
}
