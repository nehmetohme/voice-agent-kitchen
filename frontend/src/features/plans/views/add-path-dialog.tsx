import { useState } from 'react';

import { FormDialog } from '../../../shared/ui/form-dialog';
import { SelectField } from '../../../shared/ui/select-field';
import { TextField } from '../../../shared/ui/text-field';
import { PLAN_LIMITS, type Path, type Step } from '../state/models';

interface AddPathDialogProps {
  from: Step;
  /** The steps the path may lead to. */
  steps: readonly Step[];
  onAdd: (path: Omit<Path, 'id'>) => void;
}

interface Errors {
  to?: string;
  condition?: string;
}

/** Adds a path from this step: where it leads, and when to take it (FITTS-5: on the step). */
export function AddPathDialog({ from, steps, onAdd }: AddPathDialogProps) {
  const [to, setTo] = useState('');
  const [condition, setCondition] = useState('');
  const [errors, setErrors] = useState<Errors>({});
  const options = steps
    .filter((step) => step.id !== from.id)
    .map((step) => ({ value: step.id, label: step.label }));
  return (
    <FormDialog
      triggerLabel="Add path"
      title={`Add path from ${from.label}`}
      submitLabel="Add path"
      onOpenChange={(open) => {
        if (!open) return;
        setTo('');
        setCondition('');
        setErrors({});
      }}
      onSubmit={() => {
        const found: Errors = {
          ...(to === '' ? { to: 'Choose the step the path leads to.' } : {}),
          ...(condition.trim() === '' ? { condition: 'Say when to take this path.' } : {}),
        };
        setErrors(found);
        if (Object.keys(found).length > 0) return false;
        onAdd({ from: from.id, to, condition });
        return true;
      }}
    >
      <SelectField
        label="To"
        value={to}
        placeholder="Choose a step"
        options={options}
        onValueChange={setTo}
        error={errors.to}
      />
      <TextField
        label="When"
        value={condition}
        onValueChange={setCondition}
        maxLength={PLAN_LIMITS.condition}
        description="In plain words, e.g. “The caller wants to book”."
        error={errors.condition}
      />
    </FormDialog>
  );
}
