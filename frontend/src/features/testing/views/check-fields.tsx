import { Button } from '../../../shared/ui/button';
import { SelectField } from '../../../shared/ui/select-field';
import { TextField } from '../../../shared/ui/text-field';
import { SCENARIO_LIMITS } from '../state/models';
import { withCheck, withoutCheck, type ScenarioForm } from '../state/scenario-edits';

const EXPECTED = [
  { value: 'yes', label: 'Yes' },
  { value: 'no', label: 'No' },
] as const;

interface CheckFieldsProps {
  form: ScenarioForm;
  errors: readonly (string | undefined)[] | undefined;
  onChange: (form: ScenarioForm) => void;
}

/**
 * The scenario's rubric: each check is a named group of its question and the answer that
 * passes (two inputs, MILLER-2), with "Add check" after the last.
 */
export function CheckFields({ form, errors, onChange }: CheckFieldsProps) {
  const update = (index: number, change: Partial<ScenarioForm['checks'][number]>) => {
    onChange({
      ...form,
      checks: form.checks.map((check, at) => (at === index ? { ...check, ...change } : check)),
    });
  };
  return (
    <>
      {form.checks.map((check, index) => (
        // A check has no ID; its place in the rubric is its identity while editing.
        // oxlint-disable-next-line react/no-array-index-key
        <fieldset key={index} className="flex flex-col gap-3 rounded-md border border-line p-3">
          <legend className="px-1 text-sm font-medium">Check {index + 1}</legend>
          <TextField
            label="Rubric question"
            value={check.question}
            maxLength={SCENARIO_LIMITS.question}
            description="A yes/no question about the conversation."
            error={errors?.[index]}
            onValueChange={(question) => {
              update(index, { question });
            }}
          />
          <SelectField
            label="Expected answer"
            value={check.expected}
            options={EXPECTED}
            onValueChange={(value) => {
              update(index, { expected: value === 'no' ? 'no' : 'yes' });
            }}
          />
          {form.checks.length > 1 && (
            <Button
              variant="ghost"
              onClick={() => {
                onChange(withoutCheck(form, index));
              }}
            >
              Remove check {index + 1}
            </Button>
          )}
        </fieldset>
      ))}
      {form.checks.length < SCENARIO_LIMITS.rubric && (
        <Button
          variant="ghost"
          onClick={() => {
            onChange(withCheck(form));
          }}
        >
          Add check
        </Button>
      )}
    </>
  );
}
