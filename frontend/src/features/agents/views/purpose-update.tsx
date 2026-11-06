import { useQuery } from '@tanstack/react-query';
import { useId, useState } from 'react';

import { FormDialog } from '../../../shared/ui/form-dialog';
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

/** Keep or replace an edited plan: a named group of two radios (UC-1), keep chosen. */
function PlanChoice({
  replace,
  onChange,
}: {
  replace: boolean;
  onChange: (replace: boolean) => void;
}) {
  const name = useId();
  const options = [
    { value: false, label: 'Keep my plan' },
    { value: true, label: 'Replace my plan with the new starting flow' },
  ];
  return (
    <fieldset className="flex flex-col">
      <legend className="text-sm font-medium">Your plan</legend>
      {options.map((option) => (
        <label key={option.label} className="flex min-h-target cursor-pointer items-center gap-3">
          <input
            type="radio"
            name={name}
            checked={replace === option.value}
            onChange={() => {
              onChange(option.value);
            }}
            className="size-4 accent-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
          />
          <span className="text-sm">{option.label}</span>
        </label>
      ))}
    </fieldset>
  );
}

/**
 * "Update available": the agent pins an older version of its purpose. The dialog says what
 * moving to the latest version changes, and what happens to the plan; an edited plan is kept
 * unless the operator chooses to replace it. Nothing changes until they confirm.
 */
export function PurposeUpdate({ agentId }: { agentId: string }) {
  const purpose = useQuery(agentPurposeQueryOptions(agentId));
  const update = useUpdateAgentPurpose(agentId);
  const [replace, setReplace] = useState(false);
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
        if (open) setReplace(false);
      }}
      onSubmit={() => {
        update.mutate({ version: next.version, replacePlan: next.planEdited && replace });
        return true;
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
      {next.planEdited && <PlanChoice replace={replace} onChange={setReplace} />}
    </FormDialog>
  );
}
