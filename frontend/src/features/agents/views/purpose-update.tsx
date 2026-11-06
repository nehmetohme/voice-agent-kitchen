import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';

import { FormDialog } from '../../../shared/ui/form-dialog';
import { RadioGroupField } from '../../../shared/ui/radio-group-field';
import { useUpdateAgentPurpose } from '../state/agent-mutations';
import { agentPurposeQueryOptions } from '../state/agents-queries';
import type { PurposeUpdate as Update } from '../state/models';

/** Each changing area's name, as the purpose page names its sections. */
const AREAS: Record<Update['changes'][number]['area'], string> = {
  name: 'Name',
  description: 'Description',
  compliance_class: 'Compliance rules',
  persona: 'Persona',
  flow: 'Starting flow',
  tools: 'Tools',
  guardrails: 'Guardrails',
  compliance: 'Compliance additions',
};

const PLAN_OUTCOME: Record<Update['plan'], string> = {
  follows: 'No plan is saved yet, so the agent shows the new starting flow.',
  replaced: 'Your plan is still the old starting flow, so it moves to the new one.',
  kept: 'Your plan has your own edits, so it stays as it is unless you replace it.',
};

/** Keep or replace an edited plan (UC-1): keep is chosen until the operator says otherwise. */
const PLAN_CHOICES = [
  { value: 'keep', label: 'Keep my plan' },
  { value: 'replace', label: 'Replace my plan with the new starting flow' },
] as const;

type PlanChoice = (typeof PLAN_CHOICES)[number]['value'];

/**
 * "Update available": the agent pins an older version of its purpose. The dialog says what
 * moving to the latest version changes, and what happens to the plan; an edited plan is kept
 * unless the operator chooses to replace it. Nothing changes until they confirm, and the dialog
 * stays open, busy, until the update is stored; once it is, the agent has no update to offer.
 */
export function PurposeUpdate({ agentId }: { agentId: string }) {
  const purpose = useQuery(agentPurposeQueryOptions(agentId));
  const update = useUpdateAgentPurpose(agentId);
  const [plan, setPlan] = useState<PlanChoice>('keep');
  const [failed, setFailed] = useState(false);
  const next = purpose.data?.update;
  if (next === undefined || next === null) return null;
  const title = `Update to version ${String(next.version)}`;
  return (
    <FormDialog
      triggerLabel="Update available"
      title={title}
      submitLabel={title}
      pending={update.isPending}
      onOpenChange={(open) => {
        if (!open) return;
        setPlan('keep');
        setFailed(false);
      }}
      onSubmit={() => {
        setFailed(false);
        const replacePlan = next.planEdited && plan === 'replace';
        update.mutate(
          { version: next.version, replacePlan },
          {
            onError: () => {
              setFailed(true);
            },
          },
        );
        return false;
      }}
    >
      <p className="text-sm">
        {`${purpose.data?.name ?? 'Its purpose'} has a new version. Your agent uses version ${String(purpose.data?.version)} until you update.`}
      </p>
      <ul aria-label="What changes" className="flex list-disc flex-col gap-1 pl-6 text-sm">
        {next.changes.map((change) => (
          <li key={change.area}>
            <span className="font-medium">{AREAS[change.area]}</span>
            {`: ${change.summary}`}
          </li>
        ))}
      </ul>
      <p className="text-sm text-muted">{PLAN_OUTCOME[next.plan]}</p>
      {next.planEdited && (
        <RadioGroupField
          label="Your plan"
          value={plan}
          onValueChange={setPlan}
          options={PLAN_CHOICES}
        />
      )}
      {failed && (
        <p role="alert" className="text-sm text-danger">
          The agent wasn&apos;t updated. Try again.
        </p>
      )}
    </FormDialog>
  );
}
