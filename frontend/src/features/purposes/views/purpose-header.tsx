import { Link } from '@tanstack/react-router';
import { useId } from 'react';

import { Button, buttonLook } from '../../../shared/ui/button';
import { SaveStatus } from '../../../shared/ui/save-status';
import { classLabel } from '../state/choices';
import type { PurposeDetails } from '../state/models';
import { useSaveVersion } from '../state/purposes-queries';
import type { PurposeDraft } from '../state/use-purpose-draft';
import { versionState, type VersionState } from '../state/version-state';
import { CopyPurposeDialog } from './copy-purpose-dialog';

interface HeaderProps {
  purpose: PurposeDetails;
  draft: PurposeDraft;
}

function usedByText(count: number): string {
  return count === 1 ? 'Used by 1 agent' : `Used by ${String(count)} agents`;
}

/** The name, and where the purpose stands: built-in or not, its version, its rules, its use. */
function Standing({ purpose, draft, version }: HeaderProps & { version: VersionState }) {
  const { name, complianceClass } = draft.draft.overview;
  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <h2 className="truncate text-xl font-semibold" title={name}>
        {name}
      </h2>
      <p className="flex flex-wrap gap-x-3 text-sm text-muted">
        {purpose.system && <span className="font-medium text-ink">Built-in</span>}
        <span>{version.label}</span>
        <span>{classLabel(complianceClass)} rules</span>
        <span>{usedByText(purpose.usedBy)}</span>
      </p>
    </div>
  );
}

/** "Save version N", busy while it saves and explained while it has to wait. */
function SaveVersion({ id, version }: { id: string; version: VersionState }) {
  const save = useSaveVersion(id);
  const reasonId = useId();
  return (
    <>
      <Button
        variant="primary"
        disabled={!version.canSave || save.isPending}
        aria-busy={save.isPending}
        aria-describedby={version.reason === null ? undefined : reasonId}
        onClick={() => {
          save.mutate();
        }}
      >
        {`Save version ${String(version.next)}`}
      </Button>
      {version.reason !== null && (
        <p id={reasonId} className="sr-only">
          {version.reason}
        </p>
      )}
    </>
  );
}

/**
 * One row above every section: the name, where the purpose stands against its versions, and
 * its actions. The one primary action is "Save version N" (or "Duplicate" for a built-in
 * purpose); when the save has to wait, the reason shows beside the save status.
 */
export function PurposeHeader({ purpose, draft }: HeaderProps) {
  const version = versionState(purpose, draft);
  const pickerKey = purpose.system ? draft.draft.overview.complianceClass : purpose.id;
  return (
    <div className="flex flex-wrap items-center gap-x-6 gap-y-2">
      <Standing purpose={purpose} draft={draft} version={version} />
      {!purpose.system && version.reason !== null && (
        <p className="text-sm text-muted">{version.reason}</p>
      )}
      <div data-region="actions" className="flex flex-wrap items-center gap-2">
        {!purpose.system && <SaveStatus status={draft.status} onRetry={draft.retry} />}
        {purpose.latestVersion !== null && (
          <Link to="/agents/new" search={{ purpose: pickerKey }} {...buttonLook()}>
            Create agent
          </Link>
        )}
        <CopyPurposeDialog
          purposes={[{ id: purpose.id, name: draft.draft.overview.name }]}
          source={purpose.id}
          triggerLabel="Duplicate"
          triggerVariant={purpose.system ? 'primary' : 'secondary'}
          title="Duplicate purpose"
          submitLabel="Duplicate"
        />
        {!purpose.system && <SaveVersion id={purpose.id} version={version} />}
      </div>
    </div>
  );
}
