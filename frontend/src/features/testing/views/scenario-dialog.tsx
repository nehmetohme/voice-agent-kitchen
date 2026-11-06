import { useState } from 'react';

import { CheckboxField } from '../../../shared/ui/checkbox-field';
import { FormDialog } from '../../../shared/ui/form-dialog';
import { TextField } from '../../../shared/ui/text-field';
import { TextareaField } from '../../../shared/ui/textarea-field';
import { SCENARIO_LIMITS, type Scenario } from '../state/models';
import {
  formOf,
  newScenarioForm,
  newScenarioId,
  scenarioFromForm,
  type FormErrors,
  type ScenarioForm,
} from '../state/scenario-edits';
import { CheckFields } from './check-fields';

interface ScenarioDialogProps {
  /** The scenario to edit; without it, the dialog adds one. */
  scenario?: Scenario | undefined;
  /** The whole set, for defaults and unique IDs. */
  scenarios: readonly Scenario[];
  onSave: (scenario: Scenario) => void;
}

/**
 * Adds or edits a scenario: the caller (name, goal, persona, turn cap, required) and its
 * rubric checks, each group of at most seven inputs (MILLER-2). Problems show on their fields;
 * nothing is saved until the form is valid.
 */
export function ScenarioDialog({ scenario, scenarios, onSave }: ScenarioDialogProps) {
  const fresh = () => (scenario === undefined ? newScenarioForm(scenarios) : formOf(scenario));
  const [form, setForm] = useState<ScenarioForm>(fresh);
  const [errors, setErrors] = useState<FormErrors>({});
  const adding = scenario === undefined;
  const set = (change: Partial<ScenarioForm>) => {
    setForm((current) => ({ ...current, ...change }));
  };
  return (
    <FormDialog
      triggerLabel={adding ? 'Add scenario' : 'Edit'}
      title={adding ? 'Add scenario' : `Edit ${scenario.name}`}
      submitLabel={adding ? 'Add scenario' : 'Save scenario'}
      onOpenChange={(open) => {
        if (!open) return;
        setForm(fresh());
        setErrors({});
      }}
      onSubmit={() => {
        const taken = scenarios.map(({ id }) => id);
        const id = scenario?.id ?? newScenarioId(form.name, taken);
        const result = scenarioFromForm(form, id);
        if (!result.ok) {
          setErrors(result.errors);
          return false;
        }
        onSave(result.scenario);
        return true;
      }}
    >
      <fieldset className="flex flex-col gap-3">
        <legend className="text-sm font-medium">Caller</legend>
        <TextField
          label="Name"
          value={form.name}
          maxLength={SCENARIO_LIMITS.name}
          error={errors.name}
          onValueChange={(name) => {
            set({ name });
          }}
        />
        <TextareaField
          label="Caller's goal"
          value={form.goal}
          maxLength={SCENARIO_LIMITS.goal}
          description="What the simulated caller wants, and what to say or not say."
          error={errors.goal}
          onValueChange={(goal) => {
            set({ goal });
          }}
        />
        <TextField
          label="Caller's persona"
          value={form.persona}
          maxLength={SCENARIO_LIMITS.persona}
          description="Who they are and how they talk. Optional."
          error={errors.persona}
          onValueChange={(persona) => {
            set({ persona });
          }}
        />
        <TextField
          label="Turn cap"
          type="number"
          inputMode="numeric"
          min={1}
          max={SCENARIO_LIMITS.maxTurns}
          value={form.maxTurns}
          description="The most caller turns before the conversation stops."
          error={errors.maxTurns}
          onValueChange={(maxTurns) => {
            set({ maxTurns });
          }}
        />
        <CheckboxField
          label="Required"
          description="A run passes only if every required scenario passes."
          checked={form.required}
          onCheckedChange={(required) => {
            set({ required });
          }}
        />
      </fieldset>
      <CheckFields form={form} errors={errors.checks} onChange={setForm} />
    </FormDialog>
  );
}
