/**
 * Autosave rules (Parkinson, PARK-1): an edit saves on its own shortly after the last change,
 * and the page always says whether everything is saved. Pure functions; `use-autosave.ts`
 * runs them.
 */
import type { AgentSettings, FieldErrors } from '../api/agents-api';
import { checkSettings } from '../api/request-checks';

/** Quiet time after the last edit before saving: well inside the 1 s budget. */
export const AUTOSAVE_DELAY_MS = 500;

/**
 * - `saved`: everything on the page is stored.
 * - `unsaved`: an edit is waiting for the quiet time to pass.
 * - `saving`: a save is in flight.
 * - `invalid`: a field must be fixed before anything can be saved.
 * - `failed`: the last save failed; editing again, or retrying, saves again.
 */
export type SaveStatus = 'saved' | 'unsaved' | 'saving' | 'invalid' | 'failed';

const SETTINGS_KEYS = [
  'name',
  'useCase',
  'voice',
  'language',
  'jurisdiction',
  'model',
  'turnDetection',
] as const satisfies readonly (keyof AgentSettings)[];

function same(a: AgentSettings, b: AgentSettings, key: keyof AgentSettings): boolean {
  if (key === 'turnDetection') {
    return (
      a.turnDetection.silenceMs === b.turnDetection.silenceMs &&
      a.turnDetection.bargeIn === b.turnDetection.bargeIn
    );
  }
  return a[key] === b[key];
}

/** The fields of `draft` that differ from `saved`: what a save sends. */
export function changedSettings(
  saved: AgentSettings,
  draft: AgentSettings,
): Partial<AgentSettings> {
  const changes: Partial<AgentSettings> = {};
  for (const key of SETTINGS_KEYS) {
    if (!same(saved, draft, key)) Object.assign(changes, { [key]: draft[key] });
  }
  return changes;
}

/**
 * What must be fixed before the draft can be sent: the contract's request schema, so an
 * obvious mistake (an empty name, a pause out of range) never leaves the page. The server
 * checks the rest and names the field in its answer.
 */
export function validateSettings(draft: AgentSettings): FieldErrors {
  return checkSettings(draft);
}

interface StatusInput {
  dirty: boolean;
  saving: boolean;
  invalid: boolean;
  failed: boolean;
}

/** One status for the page; the most urgent wins. */
export function saveStatus({ dirty, saving, invalid, failed }: StatusInput): SaveStatus {
  if (invalid) return 'invalid';
  if (saving) return 'saving';
  if (failed) return 'failed';
  return dirty ? 'unsaved' : 'saved';
}
