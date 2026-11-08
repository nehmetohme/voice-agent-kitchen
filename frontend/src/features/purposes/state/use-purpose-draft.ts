import { useCallback, useEffect, useMemo, useState } from 'react';

import type { SaveState } from '../../../shared/ui/save-status';
import { isSendable } from '../api/purpose-mapping';
import type { PurposeDetails, PurposeSettings, SectionKey } from '../api/purpose-models';
import { fieldErrorsOf } from '../api/purposes-api';
import { changedSections } from './purpose-edits';
import { useUpdatePurpose } from './purposes-queries';

/** Quiet time after the last edit before saving: well inside the 1 s budget (PARK-1). */
const AUTOSAVE_DELAY_MS = 500;

export interface PurposeDraft {
  /** What the sections show: the operator's edits, saved or not. */
  draft: PurposeSettings;
  /** Replaces one section; the save follows on its own. */
  edit: <K extends SectionKey>(section: K, value: PurposeSettings[K]) => void;
  status: SaveState;
  /** Whether an edit is waiting to be saved. */
  dirty: boolean;
  /** The server's `{field, message}` pairs after a refused save. */
  errors: { field: string; message: string }[];
  /** Saves again after a failure. */
  retry: () => void;
}

function stateOf(input: {
  dirty: boolean;
  invalid: boolean;
  saving: boolean;
  failed: boolean;
}): SaveState {
  if (input.invalid) return 'invalid';
  if (input.saving) return 'saving';
  if (input.failed) return 'failed';
  return input.dirty ? 'unsaved' : 'saved';
}

/**
 * Holds the editable sections of `purpose` and saves each change `delayMs` after the last
 * edit, sending only the sections that changed. The draft is compared with what was last
 * sent, not with the server's normalised answer (lower-cased phrases, the locked opt-out
 * tool), so an edit never loops. A built-in purpose is never sent.
 */
export function usePurposeDraft(
  purpose: PurposeDetails,
  delayMs = AUTOSAVE_DELAY_MS,
): PurposeDraft {
  const { mutate, isPending, error } = useUpdatePurpose(purpose.id);
  const [draft, setDraft] = useState(purpose.settings);
  const [sent, setSent] = useState(purpose.settings);
  const [failedDraft, setFailedDraft] = useState<PurposeSettings | null>(null);

  const changes = useMemo(() => changedSections(sent, draft), [sent, draft]);
  const dirty = Object.keys(changes).length > 0;
  const invalid = dirty && !isSendable(changes);
  const failed = failedDraft === draft;

  useEffect(() => {
    if (purpose.system || !dirty || invalid || isPending || failed) return undefined;
    const timer = setTimeout(() => {
      mutate(changes, {
        onSuccess: () => {
          setSent(draft);
        },
        onError: () => {
          setFailedDraft(draft);
        },
      });
    }, delayMs);
    return () => {
      clearTimeout(timer);
    };
  }, [changes, delayMs, dirty, draft, failed, invalid, isPending, mutate, purpose.system]);

  const edit = useCallback(<K extends SectionKey>(section: K, value: PurposeSettings[K]) => {
    setDraft((current) => ({ ...current, [section]: value }));
  }, []);
  const retry = useCallback(() => {
    setFailedDraft(null);
  }, []);

  return {
    draft,
    edit,
    status: stateOf({ dirty, invalid, saving: isPending, failed }),
    dirty,
    errors: failed ? fieldErrorsOf(error) : [],
    retry,
  };
}
