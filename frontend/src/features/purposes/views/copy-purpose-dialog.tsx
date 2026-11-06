import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';

import { ChoiceField } from '../../../shared/ui/choice-field';
import { FormDialog } from '../../../shared/ui/form-dialog';
import { TextField } from '../../../shared/ui/text-field';
import { LIMITS, problemDetail } from '../state/models';
import { useDuplicatePurpose } from '../state/purposes-queries';

interface CopyDialogProps {
  /** The purposes to start from; the first is chosen unless `source` is given. */
  purposes: readonly { id: string; name: string }[];
  source?: string | undefined;
  triggerLabel: string;
  triggerVariant: 'primary' | 'secondary';
  title: string;
  submitLabel: string;
}

/**
 * Makes a purpose of the workspace's as a copy of another one, then opens it. Its name starts
 * as "Copy of …", ready to change.
 */
export function CopyPurposeDialog({
  purposes,
  source,
  triggerLabel,
  triggerVariant,
  title,
  submitLabel,
}: CopyDialogProps) {
  const navigate = useNavigate();
  const duplicate = useDuplicatePurpose();
  const [from, setFrom] = useState(source ?? purposes[0]?.id ?? '');
  const [name, setName] = useState('');
  const [error, setError] = useState<string | undefined>();
  const nameOf = (id: string) => purposes.find((purpose) => purpose.id === id)?.name ?? '';
  return (
    <FormDialog
      triggerLabel={triggerLabel}
      triggerVariant={triggerVariant}
      title={title}
      submitLabel={submitLabel}
      pending={duplicate.isPending}
      onOpenChange={(open) => {
        if (!open) return;
        const start = source ?? purposes[0]?.id ?? '';
        setFrom(start);
        setName(`Copy of ${nameOf(start)}`.slice(0, LIMITS.name));
        setError(undefined);
      }}
      onSubmit={() => {
        if (name.trim() === '') {
          setError('Give the purpose a name.');
          return false;
        }
        duplicate.mutate(
          { source: from, name: name.trim() },
          {
            onSuccess: (created) => {
              void navigate({ to: '/purposes/$purposeId', params: { purposeId: created.id } });
            },
            onError: (failure) => {
              setError(problemDetail(failure) ?? 'The purpose was not made. Try again.');
            },
          },
        );
        return false;
      }}
    >
      <TextField
        label="Name"
        value={name}
        onValueChange={setName}
        maxLength={LIMITS.name}
        error={error}
        autoComplete="off"
      />
      {source === undefined && (
        <ChoiceField
          label="Start from"
          value={from}
          onValueChange={(value) => {
            if (name === `Copy of ${nameOf(from)}`) setName(`Copy of ${nameOf(value)}`);
            setFrom(value);
          }}
          options={purposes.map((purpose) => ({ value: purpose.id, label: purpose.name }))}
          description="Its starting flow, tools, guardrails and compliance additions are copied."
        />
      )}
    </FormDialog>
  );
}
