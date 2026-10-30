import type { ComponentProps } from 'react';

import { Field, controlClassName } from './field';

type InputProps = Omit<ComponentProps<'input'>, 'className' | 'id' | 'onChange' | 'value'>;

interface TextFieldProps extends InputProps {
  label: string;
  value: string;
  onValueChange: (value: string) => void;
  description?: string | undefined;
  error?: string | undefined;
}

/** A labelled text input with optional help and error text. */
export function TextField({
  label,
  value,
  onValueChange,
  description,
  error,
  ...input
}: TextFieldProps) {
  return (
    <Field label={label} description={description} error={error}>
      {(ids) => (
        <input
          {...input}
          {...ids}
          value={value}
          onChange={(event) => {
            onValueChange(event.target.value);
          }}
          className={controlClassName}
        />
      )}
    </Field>
  );
}
