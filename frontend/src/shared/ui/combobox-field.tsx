import { Combobox } from '@base-ui/react/combobox';
import { useMemo } from 'react';

import { Field, controlClassName } from './field';

interface ComboboxOption {
  value: string;
  label: string;
}

interface ComboboxFieldProps {
  label: string;
  /** The chosen option's value; `''` while nothing is chosen. */
  value: string;
  onValueChange: (value: string) => void;
  options: readonly ComboboxOption[];
  placeholder?: string | undefined;
  description?: string | undefined;
  error?: string | undefined;
  disabled?: boolean | undefined;
}

/**
 * A labelled, searchable choice for long lists (Hick, HICK-3): type to filter, arrow keys to
 * move, Enter to choose. Base UI Combobox, so the input is a `role=combobox` tied to its list.
 */
export function ComboboxField({
  label,
  value,
  onValueChange,
  options,
  placeholder,
  description,
  error,
  disabled,
}: ComboboxFieldProps) {
  const items = useMemo(
    () =>
      Combobox.createItems(options, {
        getValue: (option) => option.value,
        getLabel: (option) => option.label,
      }),
    [options],
  );
  return (
    <Field label={label} description={description} error={error}>
      {(ids) => (
        <Combobox.Root
          items={items}
          value={value === '' ? null : value}
          disabled={disabled}
          onValueChange={(next) => {
            if (typeof next === 'string') onValueChange(next);
          }}
        >
          <Combobox.Input
            data-slot="combobox-input"
            {...ids}
            placeholder={placeholder}
            className={controlClassName}
          />
          <Combobox.Portal>
            <Combobox.Positioner data-slot="combobox-positioner" sideOffset={4} align="start">
              <Combobox.Popup
                data-slot="combobox-popup"
                className="flex max-h-popup min-w-step flex-col overflow-y-auto rounded-md border border-line bg-canvas p-1 text-ink shadow-md"
              >
                <Combobox.Empty>
                  <p className="p-3 text-sm text-muted">Nothing matches. Try another word.</p>
                </Combobox.Empty>
                <Combobox.List>
                  {(option: ComboboxOption) => (
                    <Combobox.Item
                      key={option.value}
                      value={option.value}
                      data-slot="combobox-item"
                      className="flex min-h-target cursor-pointer items-center rounded-sm px-3 text-sm outline-none data-highlighted:bg-hover data-selected:font-semibold"
                    >
                      {option.label}
                    </Combobox.Item>
                  )}
                </Combobox.List>
              </Combobox.Popup>
            </Combobox.Positioner>
          </Combobox.Portal>
        </Combobox.Root>
      )}
    </Field>
  );
}
