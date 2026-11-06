import { Dialog } from '@base-ui/react/dialog';
import { useState, type ReactNode } from 'react';

import { Button } from './button';
import { InsideModal, useModalOpen } from './modal-layer';

interface FormDialogProps {
  /** The button that opens the dialog. */
  triggerLabel: string;
  /** The trigger's look: `secondary` by default; `primary` when it is the screen's main action. */
  triggerVariant?: 'primary' | 'secondary' | 'ghost' | undefined;
  /** The trigger's accessible name, when its visible label is shared, e.g. "Edit book_slot". */
  triggerAriaLabel?: string | undefined;
  /** The dialog's heading and accessible name. */
  title: string;
  /** The submit button, last in the form (FITTS-4). */
  submitLabel: string;
  /** Called on submit (button or Enter); answer `true` to close the dialog. */
  onSubmit: () => boolean;
  /** Called when the dialog opens or closes, e.g. to reset its fields. */
  onOpenChange?: ((open: boolean) => void) | undefined;
  /** The submit button shows busy and is disabled while the action runs (Doherty). */
  pending?: boolean | undefined;
  /** The form's fields; the first one gets the focus when the dialog opens. */
  children: ReactNode;
}

/**
 * A small form in a modal `role=dialog`: fields first, then Cancel and the submit button. Enter
 * submits, Esc and Cancel close it without acting, and focus returns to the trigger (Jakob). A
 * form taller than the screen scrolls inside the dialog.
 */
export function FormDialog({
  triggerLabel,
  triggerVariant = 'secondary',
  triggerAriaLabel,
  title,
  submitLabel,
  onSubmit,
  onOpenChange,
  pending,
  children,
}: FormDialogProps) {
  const [open, setOpen] = useState(false);
  useModalOpen(open);
  const change = (next: boolean) => {
    setOpen(next);
    onOpenChange?.(next);
  };
  return (
    <Dialog.Root open={open} onOpenChange={change}>
      <Dialog.Trigger
        data-slot="form-dialog-trigger"
        render={<Button variant={triggerVariant} aria-label={triggerAriaLabel} />}
      >
        {triggerLabel}
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Backdrop data-slot="form-dialog-backdrop" className="fixed inset-0 bg-ink/40" />
        <Dialog.Popup
          data-slot="form-dialog-popup"
          className="fixed top-1/2 left-1/2 flex max-h-dvh w-full max-w-readable -translate-x-1/2 -translate-y-1/2 flex-col gap-6 overflow-y-auto rounded-md bg-canvas p-6 text-ink shadow-md"
        >
          <InsideModal.Provider value>
            <Dialog.Title className="text-lg font-semibold">{title}</Dialog.Title>
            <form
              noValidate
              className="flex flex-col gap-6"
              onSubmit={(event) => {
                event.preventDefault();
                if (onSubmit()) change(false);
              }}
            >
              <div className="flex flex-col gap-4">{children}</div>
              <div data-region="actions" className="flex justify-end gap-2">
                <Dialog.Close render={<Button />}>Cancel</Dialog.Close>
                <Button
                  variant="primary"
                  type="submit"
                  disabled={pending === true}
                  aria-busy={pending === true}
                >
                  {submitLabel}
                </Button>
              </div>
            </form>
          </InsideModal.Provider>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
