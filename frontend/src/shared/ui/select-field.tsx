import { Field, controlClassName } from './field';

interface SelectOption {
  value: string;
  label: string;
}

interface SelectFieldProps {
  label: string;
  value: string;
  onValueChange: (value: string) => void;
  /** At most 7 (Hick, HICK-3); a longer list needs a searchable combobox instead. */
  options: readonly SelectOption[];
  /** Shown first and not selectable, while nothing is chosen yet. */
  placeholder?: string | undefined;
  description?: string | undefined;
  error?: string | undefined;
  disabled?: boolean | undefined;
}

/**
 * A labelled native select: the platform's own keyboard, touch and screen-reader behaviour
 * (Jakob). For short, fixed lists only.
 */
export function SelectField({
  label,
  value,
  onValueChange,
  options,
  placeholder,
  description,
  error,
  disabled,
}: SelectFieldProps) {
  return (
    <Field label={label} description={description} error={error}>
      {(ids) => (
        <select
          {...ids}
          value={value}
          disabled={disabled}
          onChange={(event) => {
            onValueChange(event.target.value);
          }}
          className={controlClassName}
        >
          {placeholder !== undefined && (
            <option value="" disabled>
              {placeholder}
            </option>
          )}
          {options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      )}
    </Field>
  );
}
