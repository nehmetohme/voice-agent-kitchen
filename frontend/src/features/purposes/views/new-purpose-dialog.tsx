import type { PurposeListItem } from '../state/models';
import { CopyPurposeDialog } from './copy-purpose-dialog';

/** "New purpose": a copy of a built-in purpose or of one of the workspace's. */
export function NewPurposeDialog({ purposes }: { purposes: readonly PurposeListItem[] }) {
  return (
    <CopyPurposeDialog
      purposes={purposes}
      triggerLabel="New purpose"
      triggerVariant="primary"
      title="New purpose"
      submitLabel="Create purpose"
    />
  );
}
