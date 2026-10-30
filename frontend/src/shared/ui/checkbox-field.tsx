import { useId } from 'react';

interface CheckboxFieldProps {
  label: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  description?: string | undefined;
}

/** A labelled native checkbox. The whole label row is the 44 px target (Fitts). */
export function CheckboxField({
  label,
  checked,
  onCheckedChange,
  description,
}: CheckboxFieldProps) {
  const id = useId();
  const descriptionId = `${id}-description`;
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={id} className="flex min-h-target cursor-pointer items-center gap-3">
        <input
          id={id}
          type="checkbox"
          checked={checked}
          aria-describedby={description === undefined ? undefined : descriptionId}
          onChange={(event) => {
            onCheckedChange(event.target.checked);
          }}
          className="size-4 accent-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
        />
        <span className="text-sm font-medium">{label}</span>
      </label>
      {description !== undefined && (
        <p id={descriptionId} className="text-sm text-muted">
          {description}
        </p>
      )}
    </div>
  );
}
