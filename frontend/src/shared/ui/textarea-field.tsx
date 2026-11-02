import { Field, controlClassName } from './field';

interface TextareaFieldProps {
  label: string;
  value: string;
  onValueChange: (value: string) => void;
  description?: string | undefined;
  error?: string | undefined;
  maxLength?: number | undefined;
}

/** A labelled multi-line text input, for prompts and scripts. */
export function TextareaField({
  label,
  value,
  onValueChange,
  description,
  error,
  maxLength,
}: TextareaFieldProps) {
  return (
    <Field label={label} description={description} error={error}>
      {(ids) => (
        <textarea
          {...ids}
          rows={4}
          value={value}
          maxLength={maxLength}
          onChange={(event) => {
            onValueChange(event.target.value);
          }}
          className={`${controlClassName} py-2`}
        />
      )}
    </Field>
  );
}
