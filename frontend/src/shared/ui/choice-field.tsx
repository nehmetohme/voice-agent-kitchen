import { ComboboxField } from './combobox-field';
import { SelectField } from './select-field';

/** The longest list a native select shows (Hick, HICK-3); a longer one is searchable. */
export const MAX_SELECT_OPTIONS = 7;

interface ChoiceFieldProps {
  label: string;
  value: string;
  onValueChange: (value: string) => void;
  options: readonly { value: string; label: string }[];
  placeholder?: string | undefined;
  description?: string | undefined;
  error?: string | undefined;
  disabled?: boolean | undefined;
}

/**
 * One choice from a list that grows, such as the workspace's purposes: a native select while
 * the list is short (≤ 7, Jakob), a searchable combobox once it is longer (HICK-3).
 */
export function ChoiceField({ options, ...field }: ChoiceFieldProps) {
  return options.length > MAX_SELECT_OPTIONS ? (
    <ComboboxField {...field} options={options} />
  ) : (
    <SelectField {...field} options={options} />
  );
}
