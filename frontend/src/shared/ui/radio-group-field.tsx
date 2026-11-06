import { useId } from 'react';

interface RadioGroupFieldProps<T extends string> {
  /** The group's name: its legend. */
  label: string;
  value: T;
  onValueChange: (value: T) => void;
  /** A few fixed choices (≤ 7, HICK-3); more belong in a select or a combobox. */
  options: readonly { value: T; label: string }[];
}

/**
 * One choice from a few, all in view: a named group (`fieldset` and `legend`) of native radios.
 * Each label row is the 44 px target (Fitts), and arrow keys move between the choices.
 */
export function RadioGroupField<T extends string>({
  label,
  value,
  onValueChange,
  options,
}: RadioGroupFieldProps<T>) {
  const name = useId();
  return (
    <fieldset className="flex flex-col">
      <legend className="text-sm font-medium">{label}</legend>
      {options.map((option) => (
        <label
          key={option.value}
          className="flex min-h-target cursor-pointer items-center gap-3 has-disabled:cursor-not-allowed"
        >
          <input
            type="radio"
            name={name}
            value={option.value}
            checked={value === option.value}
            onChange={() => {
              onValueChange(option.value);
            }}
            className="size-4 accent-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
          />
          <span className="text-sm">{option.label}</span>
        </label>
      ))}
    </fieldset>
  );
}
