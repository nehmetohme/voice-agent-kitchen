/**
 * Which layer is in front. While a modal dialog is open it is the screen, so its submit is the
 * one primary action (Von Restorff, HICK-1): a primary `Button` behind it looks secondary until
 * the dialog closes. Dialogs register while open and mark their content as inside the modal.
 */
import { createContext, useContext, useEffect, useSyncExternalStore } from 'react';

let openModals = 0;
const listeners = new Set<() => void>();

function emit(): void {
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Counts the calling dialog as open while `open` holds. */
export function useModalOpen(open: boolean): void {
  useEffect(() => {
    if (!open) return undefined;
    openModals += 1;
    emit();
    return () => {
      openModals -= 1;
      emit();
    };
  }, [open]);
}

/** Whether something is inside an open modal dialog (provided by the dialogs). */
export const InsideModal = createContext(false);

/** Whether a modal dialog covers the calling component. */
export function useCoveredByModal(): boolean {
  const inside = useContext(InsideModal);
  const anyOpen = useSyncExternalStore(
    subscribe,
    () => openModals > 0,
    () => false,
  );
  return anyOpen && !inside;
}
