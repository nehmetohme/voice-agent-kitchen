import { Button } from '../../../shared/ui/button';
import { CheckboxField } from '../../../shared/ui/checkbox-field';
import { SelectField } from '../../../shared/ui/select-field';
import { TextField } from '../../../shared/ui/text-field';
import { INPUT_TYPES, choose } from '../state/choices';
import { LIMITS, type ToolInput } from '../state/models';

interface ToolInputFieldsProps {
  inputs: readonly ToolInput[];
  onChange: (inputs: ToolInput[]) => void;
  error?: string | undefined;
}

/** A tool's inputs, one named group each: its name, type, what it means, and whether it is needed. */
export function ToolInputFields({ inputs, onChange, error }: ToolInputFieldsProps) {
  const put = (at: number, input: ToolInput) => {
    onChange(inputs.map((existing, index) => (index === at ? input : existing)));
  };
  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm font-medium">Inputs</p>
      {inputs.length === 0 && (
        <p className="text-sm text-muted">No inputs: the tool is called without any.</p>
      )}
      {error !== undefined && <p className="text-sm text-danger">{error}</p>}
      {inputs.map((input, at) => {
        const number = String(at + 1);
        return (
          <fieldset
            // Inputs have no ID of their own and are only appended or removed.
            // oxlint-disable-next-line react/no-array-index-key
            key={at}
            className="grid grid-cols-2 items-end gap-3 rounded-md border border-line p-3"
          >
            <legend className="px-1 text-sm font-medium">{`Input ${number}`}</legend>
            <TextField
              label="Name"
              value={input.name}
              onValueChange={(name) => {
                put(at, { ...input, name });
              }}
              autoComplete="off"
            />
            <SelectField
              label="Type"
              value={input.type}
              onValueChange={(value) => {
                const type = choose(INPUT_TYPES, value);
                if (type !== undefined) put(at, { ...input, type });
              }}
              options={INPUT_TYPES}
            />
            <div className="col-span-2">
              <TextField
                label="Description"
                value={input.description}
                onValueChange={(description) => {
                  put(at, { ...input, description });
                }}
                maxLength={LIMITS.inputDescription}
                autoComplete="off"
              />
            </div>
            <CheckboxField
              label="Required"
              checked={input.required}
              onCheckedChange={(required) => {
                put(at, { ...input, required });
              }}
            />
            <Button
              variant="ghost"
              aria-label={`Remove input ${number}`}
              onClick={() => {
                onChange(inputs.filter((_, index) => index !== at));
              }}
            >
              Remove
            </Button>
          </fieldset>
        );
      })}
    </div>
  );
}
