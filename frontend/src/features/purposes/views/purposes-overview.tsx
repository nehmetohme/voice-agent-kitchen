import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';

import { LoadFailed, Loading } from '../../../shared/ui/load-states';
import { classLabel } from '../state/choices';
import type { PurposeListItem } from '../state/models';
import { purposesQueryOptions } from '../state/purposes-queries';
import { NewPurposeDialog } from './new-purpose-dialog';

function versionText(purpose: PurposeListItem): string {
  if (purpose.latestVersion === null) return 'Not saved as a version yet';
  const version = `Version ${String(purpose.latestVersion)}`;
  return purpose.unsavedChanges ? `${version}, with changes` : version;
}

function usedByText(count: number): string {
  return count === 1 ? 'Used by 1 agent' : `Used by ${String(count)} agents`;
}

/** One purpose: its name links to its page; what it is for and how it stands, beside it. */
function PurposeRow({ purpose }: { purpose: PurposeListItem }) {
  return (
    <li className="flex flex-wrap items-center justify-between gap-x-6 gap-y-1 rounded-md border border-line p-4">
      <div className="flex min-w-0 flex-col gap-1">
        <Link
          to="/purposes/$purposeId"
          params={{ purposeId: purpose.id }}
          className="min-h-target content-center font-semibold underline-offset-4 hover:underline"
        >
          {purpose.name}
        </Link>
        <p className="text-sm text-muted">
          {purpose.system ? 'Built-in' : classLabel(purpose.complianceClass)}
          {purpose.description === '' ? '' : ` · ${purpose.description}`}
        </p>
      </div>
      <p className="flex gap-4 text-sm text-muted">
        <span>{versionText(purpose)}</span>
        <span>{usedByText(purpose.usedBy)}</span>
      </p>
    </li>
  );
}

/**
 * Every purpose: the six built-in ones first (read-only, to duplicate), then the workspace's
 * own. One primary action, "New purpose".
 */
export function PurposesOverview() {
  const purposes = useQuery(purposesQueryOptions());
  if (purposes.isPending) return <Loading label="Loading purposes…" />;
  if (purposes.isError) {
    return (
      <LoadFailed
        what="the purposes"
        onRetry={() => {
          void purposes.refetch();
        }}
      />
    );
  }
  const own = purposes.data.filter((purpose) => !purpose.system);
  return (
    <div className="flex max-w-readable flex-col gap-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <p className="text-muted">
          What agents are for and how they may work: a starting flow, tools and guardrails.
        </p>
        <div data-region="actions">
          <NewPurposeDialog purposes={purposes.data} />
        </div>
      </div>
      <ul aria-label="Purposes" className="flex flex-col gap-2">
        {purposes.data.map((purpose) => (
          <PurposeRow key={purpose.id} purpose={purpose} />
        ))}
      </ul>
      {own.length === 0 && (
        <p className="text-sm text-muted">
          Built-in purposes are read-only. Open one and choose Duplicate, or start a new purpose, to
          make your own.
        </p>
      )}
    </div>
  );
}
