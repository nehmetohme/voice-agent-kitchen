import { ChevronRight } from 'lucide-react';

import { CheckboxField } from '../../../shared/ui/checkbox-field';
import { SelectField } from '../../../shared/ui/select-field';
import { TextField } from '../../../shared/ui/text-field';
import { MODELS, toModel } from '../state/choices';
import { LIMITS, type AgentSettings, type FieldErrors } from '../state/models';
import type { Autosave } from '../state/use-autosave';

interface AdvancedSettingsProps {
  draft: AgentSettings;
  edit: Autosave['edit'];
  errors: FieldErrors;
}

/**
 * Model and timing, behind the form's one "Advanced" disclosure (Tesler, TES-3): the defaults
 * suit most agents, so these stay out of the way until someone needs them.
 */
export function AdvancedSettings({ draft, edit, errors }: AdvancedSettingsProps) {
  const turn = draft.turnDetection;
  return (
    <details className="group rounded-md border border-line">
      <summary className="flex min-h-target cursor-pointer items-center gap-2 rounded-md px-4 font-medium focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus">
        <ChevronRight aria-hidden className="size-4 transition-transform group-open:rotate-90" />
        Advanced
        <span className="text-sm font-normal text-muted">Model and timing</span>
      </summary>
      <fieldset className="flex flex-col gap-4 px-4 pb-4">
        <legend className="sr-only">Model and timing</legend>
        <SelectField
          label="Model"
          value={draft.model}
          onValueChange={(value) => {
            const model = toModel(value);
            if (model !== undefined) edit('model', model);
          }}
          options={MODELS}
        />
        <TextField
          label="Pause before replying (ms)"
          type="number"
          inputMode="numeric"
          min={LIMITS.silenceMinMs}
          max={LIMITS.silenceMaxMs}
          step={50}
          value={Number.isNaN(turn.silenceMs) ? '' : String(turn.silenceMs)}
          onValueChange={(value) => {
            edit('turnDetection', {
              ...turn,
              silenceMs: value === '' ? Number.NaN : Number(value),
            });
          }}
          description="How long the caller is silent before the agent answers. 500 ms suits most calls."
          error={errors.turnDetection}
        />
        <CheckboxField
          label="Callers can interrupt the agent"
          checked={turn.bargeIn}
          onCheckedChange={(bargeIn) => {
            edit('turnDetection', { ...turn, bargeIn });
          }}
        />
      </fieldset>
    </details>
  );
}
