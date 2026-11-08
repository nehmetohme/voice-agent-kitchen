import { SelectField } from '../../../shared/ui/select-field';
import { TextField } from '../../../shared/ui/text-field';
import { TextareaField } from '../../../shared/ui/textarea-field';
import { CLASSES, choose } from '../state/choices';
import { LIMITS, type OverviewSettings } from '../state/models';

interface OverviewSectionProps {
  overview: OverviewSettings;
  onChange: (overview: OverviewSettings) => void;
  readOnly: boolean;
  /** The server's message per field, after a refused save. */
  errors: Record<keyof OverviewSettings, string | undefined>;
}

/** What the purpose is called, what it is for, which rules apply, and the agent's persona. */
export function OverviewSection({ overview, onChange, readOnly, errors }: OverviewSectionProps) {
  const set = <K extends keyof OverviewSettings>(field: K, value: OverviewSettings[K]) => {
    onChange({ ...overview, [field]: value });
  };
  return (
    <fieldset disabled={readOnly} className="flex max-w-readable flex-col gap-4">
      <legend className="sr-only">Overview</legend>
      <TextField
        label="Name"
        value={overview.name}
        onValueChange={(name) => {
          set('name', name);
        }}
        maxLength={LIMITS.name}
        error={errors.name}
        autoComplete="off"
      />
      <TextareaField
        label="Description"
        value={overview.description}
        onValueChange={(description) => {
          set('description', description);
        }}
        maxLength={LIMITS.description}
        description="What agents with this purpose are for, in a sentence or two."
        error={errors.description}
      />
      <SelectField
        label="Compliance rules"
        value={overview.complianceClass}
        onValueChange={(value) => {
          const complianceClass = choose(CLASSES, value);
          if (complianceClass !== undefined) set('complianceClass', complianceClass);
        }}
        options={CLASSES}
        description="Which rule pack family applies; with each agent's jurisdiction it picks the pack."
        error={errors.complianceClass}
      />
      <TextareaField
        label="Persona"
        value={overview.persona}
        onValueChange={(persona) => {
          set('persona', persona);
        }}
        maxLength={LIMITS.persona}
        description="How agents with this purpose talk and what their job is. Their name and language come from each agent."
        error={errors.persona}
      />
    </fieldset>
  );
}
