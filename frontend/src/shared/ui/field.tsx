import { useId, type ReactNode } from 'react';

import { cn } from '../lib/cn';

/** Classes shared by text inputs and selects: the 44 px target and the focus ring. */
export const controlClassName = cn(
  'min-h-target w-full rounded-md border border-line bg-canvas px-3 text-base text-ink',
  'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus',
  'disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-danger',
);

interface FieldProps {
  label: string;
  /** Help under the control. */
  description?: string | undefined;
  /** What to fix; shown under the control and announced with it. */
  error?: string | undefined;
  /** Renders the control with the IDs that tie it to its label and messages. */
  children: (ids: FieldIds) => ReactNode;
}

interface FieldIds {
  id: string;
  'aria-describedby': string | undefined;
  'aria-invalid': boolean | undefined;
}

/** A label, a control, then its help and error text directly under it (proximity). */
export function Field({ label, description, error, children }: FieldProps) {
  const id = useId();
  const descriptionId = `${id}-description`;
  const errorId = `${id}-error`;
  const describedBy = [description && descriptionId, error && errorId].filter(Boolean).join(' ');
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={id} className="text-sm font-medium">
        {label}
      </label>
      {children({
        id,
        'aria-describedby': describedBy === '' ? undefined : describedBy,
        'aria-invalid': error === undefined ? undefined : true,
      })}
      {description && (
        <p id={descriptionId} className="text-sm text-muted">
          {description}
        </p>
      )}
      {error && (
        <p id={errorId} className="text-sm text-danger">
          {error}
        </p>
      )}
    </div>
  );
}
