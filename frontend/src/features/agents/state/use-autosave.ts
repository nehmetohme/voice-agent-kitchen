import { useCallback, useEffect, useMemo, useState } from 'react';

import {
  fieldErrorsOf,
  type AgentDetails,
  type AgentSettings,
  type FieldErrors,
} from '../api/agents-api';
import { useUpdateAgent } from './agent-mutations';
import {
  AUTOSAVE_DELAY_MS,
  changedSettings,
  saveStatus,
  validateSettings,
  type SaveStatus,
} from './autosave';

export interface Autosave {
  /** What the form shows: the operator's edits, saved or not. */
  draft: AgentSettings;
  /** Changes one field; the save follows on its own. */
  edit: <K extends keyof AgentSettings>(field: K, value: AgentSettings[K]) => void;
  status: SaveStatus;
  /** Field → what to fix (from the page's checks or the server's). */
  errors: FieldErrors;
  /** Saves again after a failure. */
  retry: () => void;
}

/**
 * Holds the editable draft of `agent` and saves each change `delayMs` after the last edit.
 * The draft is compared with what was last *sent*, not with the server's normalised answer,
 * so typing a trailing space or a lower-case code never loops.
 */
export function useAutosave(agent: AgentDetails, delayMs = AUTOSAVE_DELAY_MS): Autosave {
  const { mutate, isPending, error } = useUpdateAgent(agent.id);
  const [draft, setDraft] = useState(agent.settings);
  const [saved, setSaved] = useState(agent.settings);
  const [failedDraft, setFailedDraft] = useState<AgentSettings | null>(null);

  const changes = useMemo(() => changedSettings(saved, draft), [saved, draft]);
  const checks = useMemo(() => validateSettings(draft), [draft]);
  const dirty = Object.keys(changes).length > 0;
  const invalid = Object.keys(checks).length > 0;
  const failed = failedDraft === draft;

  useEffect(() => {
    if (!dirty || invalid || isPending || failed) return undefined;
    const timer = setTimeout(() => {
      mutate(changes, {
        onSuccess: () => {
          setSaved(draft);
        },
        onError: () => {
          setFailedDraft(draft);
        },
      });
    }, delayMs);
    return () => {
      clearTimeout(timer);
    };
  }, [changes, delayMs, dirty, draft, failed, invalid, isPending, mutate]);

  const edit = useCallback(<K extends keyof AgentSettings>(field: K, value: AgentSettings[K]) => {
    setDraft((current) => ({ ...current, [field]: value }));
  }, []);
  const retry = useCallback(() => {
    setFailedDraft(null);
  }, []);

  return {
    draft,
    edit,
    status: saveStatus({ dirty, saving: isPending, invalid, failed }),
    errors: { ...(failed ? fieldErrorsOf(error) : {}), ...checks },
    retry,
  };
}
