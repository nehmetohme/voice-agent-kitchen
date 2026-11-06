import { useState } from 'react';

import { FormDialog } from '../../../shared/ui/form-dialog';
import { SelectField } from '../../../shared/ui/select-field';
import { TextField } from '../../../shared/ui/text-field';
import { TextareaField } from '../../../shared/ui/textarea-field';
import { ACTIONS, choose } from '../state/choices';
import { LIMITS, type Escalation } from '../state/models';
import { newEscalation } from '../state/purpose-edits';

/**
 * Adds an escalation rule: when (a yes/no question the decision engine answers about the
 * call) and what to do then. Acting on it comes with M4b; the question never holds up a reply.
 */
export function EscalationDialog({ onAdd }: { onAdd: (rule: Escalation) => void }) {
  const [rule, setRule] = useState<Escalation>(newEscalation);
  const [errors, setErrors] = useState<{ when?: string; message?: string }>({});
  return (
    <FormDialog
      triggerLabel="Add escalation rule"
      title="Add escalation rule"
      submitLabel="Add rule"
      onOpenChange={(open) => {
        if (!open) return;
        setRule(newEscalation());
        setErrors({});
      }}
      onSubmit={() => {
        const found: { when?: string; message?: string } = {};
        if (rule.when.trim() === '') found.when = 'Ask a yes/no question.';
        if (rule.action === 'end_call' && rule.message.trim() === '') {
          found.message = 'Say what the agent says before it ends the call.';
        }
        setErrors(found);
        if (Object.keys(found).length > 0) return false;
        onAdd({ ...rule, when: rule.when.trim(), message: rule.message.trim() });
        return true;
      }}
    >
      <TextField
        label="When"
        value={rule.when}
        onValueChange={(when) => {
          setRule((current) => ({ ...current, when }));
        }}
        maxLength={LIMITS.question}
        placeholder="Does the caller ask for a human?"
        description="A yes/no question about the call."
        error={errors.when}
        autoComplete="off"
      />
      <SelectField
        label="Then"
        value={rule.action}
        onValueChange={(value) => {
          const action = choose(ACTIONS, value);
          if (action !== undefined) setRule((current) => ({ ...current, action }));
        }}
        options={ACTIONS}
      />
      {rule.action === 'end_call' && (
        <TextareaField
          label="Message"
          value={rule.message}
          onValueChange={(message) => {
            setRule((current) => ({ ...current, message }));
          }}
          maxLength={LIMITS.message}
          description="Said word for word before the call ends."
          error={errors.message}
        />
      )}
    </FormDialog>
  );
}
