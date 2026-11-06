/**
 * Where a purpose stands against its versions, in words, and whether "Save version N" can run
 * now. Pure: the header shows it.
 */
import type { SaveState } from '../../../shared/ui/save-status';
import type { PurposeDetails } from '../api/purpose-models';

export interface VersionState {
  /** "Not saved as a version yet", "Changes since version 2" or "Version 2". */
  label: string;
  /** The number the next save makes. */
  next: number;
  canSave: boolean;
  /** Why the save waits, when it does. */
  reason: string | null;
}

type Standing = Pick<PurposeDetails, 'latestVersion' | 'unsavedChanges' | 'problems' | 'system'>;

function labelOf(latest: number | null, changed: boolean): string {
  if (latest === null) return 'Not saved as a version yet';
  return changed ? `Changes since version ${String(latest)}` : `Version ${String(latest)}`;
}

function problemsReason(count: number): string | null {
  if (count === 0) return null;
  return count === 1 ? 'Fix 1 problem first.' : `Fix ${String(count)} problems first.`;
}

function reasonOf(purpose: Standing, changed: boolean, status: SaveState): string | null {
  if (purpose.system) return 'Built-in purposes are read-only.';
  if (!changed) return `Nothing changed since version ${String(purpose.latestVersion)}.`;
  if (status !== 'saved') return 'Your last change is still being saved.';
  return problemsReason(purpose.problems.length);
}

/** The version state of `purpose` while its draft autosaves with `status`. */
export function versionState(
  purpose: Standing,
  { status, dirty }: { status: SaveState; dirty: boolean },
): VersionState {
  const latest = purpose.latestVersion;
  const changed = latest === null || dirty || purpose.unsavedChanges;
  const reason = reasonOf(purpose, changed, status);
  return {
    label: labelOf(latest, changed),
    next: (latest ?? 0) + 1,
    canSave: reason === null,
    reason,
  };
}
