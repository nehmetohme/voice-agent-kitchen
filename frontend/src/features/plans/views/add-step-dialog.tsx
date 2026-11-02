import { useState } from 'react';

import { FormDialog } from '../../../shared/ui/form-dialog';
import { SelectField } from '../../../shared/ui/select-field';
import { TextField } from '../../../shared/ui/text-field';
import { PLAN_LIMITS, type Step, type StepKind } from '../state/models';
import { ADDABLE_KINDS, STEP_KINDS } from './step-kinds';

const KIND_OPTIONS = ADDABLE_KINDS.map((kind) => ({ value: kind, label: STEP_KINDS[kind].label }));

function isAddable(value: string): value is StepKind {
  return ADDABLE_KINDS.some((kind) => kind === value);
}

/** Adds a step: a name and a kind. Its paths come next, from the step itself. */
export function AddStepDialog({ onAdd }: { onAdd: (step: Pick<Step, 'label' | 'kind'>) => void }) {
  const [label, setLabel] = useState('');
  const [kind, setKind] = useState<StepKind>('say');
  const [error, setError] = useState<string | undefined>();
  return (
    <FormDialog
      triggerLabel="Add step"
      title="Add step"
      submitLabel="Add step"
      onOpenChange={(open) => {
        if (!open) return;
        setLabel('');
        setKind('say');
        setError(undefined);
      }}
      onSubmit={() => {
        const name = label.trim();
        if (name === '') {
          setError('Give the step a name.');
          return false;
        }
        onAdd({ label: name, kind });
        return true;
      }}
    >
      <TextField
        label="Name"
        value={label}
        onValueChange={setLabel}
        maxLength={PLAN_LIMITS.label}
        error={error}
      />
      <SelectField
        label="Kind"
        value={kind}
        options={KIND_OPTIONS}
        onValueChange={(value) => {
          if (isAddable(value)) setKind(value);
        }}
      />
    </FormDialog>
  );
}
