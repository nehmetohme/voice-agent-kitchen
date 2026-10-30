import { AlertDialog } from '@base-ui/react/alert-dialog';

import { Button } from './button';

interface ConfirmDialogProps {
  /** The danger button that opens the dialog. */
  triggerLabel: string;
  title: string;
  /** What happens, in one or two sentences. */
  description: string;
  /** The danger button that confirms. */
  confirmLabel: string;
  onConfirm: () => void;
  /** Shows the confirm button busy while the action runs (Doherty). */
  pending?: boolean | undefined;
}

/**
 * A destructive action behind a `role=alertdialog` confirmation (VR-4). Esc and Cancel close
 * it without acting; focus returns to the trigger.
 */
export function ConfirmDialog({
  triggerLabel,
  title,
  description,
  confirmLabel,
  onConfirm,
  pending = false,
}: ConfirmDialogProps) {
  return (
    <AlertDialog.Root>
      <AlertDialog.Trigger data-slot="confirm-trigger" render={<Button variant="danger" />}>
        {triggerLabel}
      </AlertDialog.Trigger>
      <AlertDialog.Portal>
        <AlertDialog.Backdrop data-slot="confirm-backdrop" className="fixed inset-0 bg-ink/40" />
        <AlertDialog.Popup
          data-slot="confirm-popup"
          className="fixed top-1/2 left-1/2 flex w-full max-w-readable -translate-x-1/2 -translate-y-1/2 flex-col gap-6 rounded-md bg-canvas p-6 text-ink shadow-md"
        >
          <div className="flex flex-col gap-2">
            <AlertDialog.Title className="text-lg font-semibold">{title}</AlertDialog.Title>
            <AlertDialog.Description className="text-muted">{description}</AlertDialog.Description>
          </div>
          <div data-region="actions" className="flex justify-end gap-2">
            <AlertDialog.Close render={<Button />}>Cancel</AlertDialog.Close>
            <Button variant="danger" onClick={onConfirm} disabled={pending} aria-busy={pending}>
              {confirmLabel}
            </Button>
          </div>
        </AlertDialog.Popup>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}
