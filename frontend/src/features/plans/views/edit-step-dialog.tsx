import { useState } from 'react';

import { FormDialog } from '../../../shared/ui/form-dialog';
import { TextField } from '../../../shared/ui/text-field';
import { TextareaField } from '../../../shared/ui/textarea-field';
import { PLAN_LIMITS, type LockedText, type Step } from '../state/models';
import { joinOpening, splitOpening } from '../state/plan-edits';
import { LockedOpening } from './locked-opening';

interface EditStepDialogProps {
  step: Step;
  /** The rule pack's locked texts: a start step always opens with them. */
  locked: readonly LockedText[];
  onSave: (changes: Pick<Step, 'label' | 'text'>) => void;
}

/**
 * Renames a step and changes what the agent says or does there. On the start step only the
 * greeting is editable; the locked texts stay in front of it, word for word.
 */
export function EditStepDialog({ step, locked, onSave }: EditStepDialogProps) {
  const isStart = step.kind === 'start';
  const [label, setLabel] = useState(step.label);
  const [text, setText] = useState('');
  const [error, setError] = useState<string | undefined>();
  return (
    <FormDialog
      triggerLabel="Edit step"
      title={`Edit ${step.label}`}
      submitLabel="Save step"
      onOpenChange={(open) => {
        if (!open) return;
        setLabel(step.label);
        setText(isStart ? splitOpening(step.text, locked).greeting : step.text);
        setError(undefined);
      }}
      onSubmit={() => {
        if (label.trim() === '') {
          setError('Give the step a name.');
          return false;
        }
        onSave({ label: label.trim(), text: isStart ? joinOpening(locked, text) : text });
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
      {isStart && <LockedOpening locked={locked} />}
      <TextareaField
        label={isStart ? 'Greeting' : 'What the agent says or does'}
        value={text}
        onValueChange={setText}
        maxLength={PLAN_LIMITS.text}
        description={isStart ? 'Said after the locked texts.' : undefined}
      />
    </FormDialog>
  );
}
