import { Lock } from 'lucide-react';
import { useState } from 'react';

import { SelectField } from '../../../shared/ui/select-field';
import { TextField } from '../../../shared/ui/text-field';
import { TextListField } from '../../../shared/ui/text-list-field';
import { LIMITS, type Baseline, type ComplianceSettings } from '../state/models';

interface ComplianceSectionProps {
  compliance: ComplianceSettings;
  onChange: (compliance: ComplianceSettings) => void;
  baselines: readonly Baseline[];
  readOnly: boolean;
}

function days(value: string): number | null {
  const number = Number(value);
  return value.trim() === '' || !Number.isFinite(number) ? null : number;
}

/** The pack's baseline, read-only: it always applies, whatever the purpose adds. */
function PackBaseline({ baseline }: { baseline: Baseline }) {
  const { retention, effective } = baseline;
  return (
    <section
      aria-label="Rule pack baseline"
      className="flex flex-col gap-3 rounded-md border border-line bg-surface p-4"
    >
      <p className="flex items-center gap-2 text-sm font-medium">
        <Lock aria-hidden className="size-4" />
        {`Rule pack ${baseline.rulePack}: always applies`}
      </p>
      <p className="text-sm">{baseline.lockedTexts.join(' ')}</p>
      <p className="text-sm text-muted">{`Opt-out phrases: ${baseline.optOutPhrases.join(', ')}`}</p>
      <p className="text-sm text-muted">{`On opt-out: ${baseline.optOutConfirmation}`}</p>
      <p className="text-sm text-muted">{`Never claims: ${baseline.forbiddenClaims.join(' ')}`}</p>
      <p className="text-sm text-muted">
        {`Kept: recordings ${String(retention.recordingsDays)} days, transcripts ${String(retention.transcriptsDays)} days, audit ${String(retention.auditDays)} days.`}
      </p>
      <p className="text-sm font-medium">
        {`With this purpose: recordings ${String(effective.retention.recordingsDays)} days, transcripts ${String(effective.retention.transcriptsDays)} days, audit ${String(effective.retention.auditDays)} days; ${String(effective.optOutPhrases.length)} opt-out phrases, ${String(effective.forbiddenClaims.length)} forbidden claims.`}
      </p>
    </section>
  );
}

/** Shorter recording and transcript retention: tighten-only, so a longer period has no effect. */
function RetentionCaps({
  compliance,
  onChange,
  readOnly,
}: Omit<ComplianceSectionProps, 'baselines'>) {
  const set = <K extends keyof ComplianceSettings>(field: K, value: ComplianceSettings[K]) => {
    onChange({ ...compliance, [field]: value });
  };
  return (
    <fieldset disabled={readOnly} className="grid grid-cols-2 gap-3">
      <legend className="mb-2 text-sm font-medium">Keep for at most (days)</legend>
      <TextField
        label="Recordings"
        type="number"
        min={1}
        max={LIMITS.days}
        value={compliance.recordingsDays === null ? '' : String(compliance.recordingsDays)}
        onValueChange={(value) => {
          set('recordingsDays', days(value));
        }}
        description="Empty: the pack's period."
      />
      <TextField
        label="Transcripts"
        type="number"
        min={1}
        max={LIMITS.days}
        value={compliance.transcriptsDays === null ? '' : String(compliance.transcriptsDays)}
        onValueChange={(value) => {
          set('transcriptsDays', days(value));
        }}
        description="A longer period has no effect."
      />
    </fieldset>
  );
}

/**
 * Compliance is tighten-only: the pack's locked texts, opt-out handling, forbidden claims and
 * audit period always apply. A purpose can add phrases and claims and keep recordings and
 * transcripts for less time, never more.
 */
export function ComplianceSection({
  compliance,
  onChange,
  baselines,
  readOnly,
}: ComplianceSectionProps) {
  const [jurisdiction, setJurisdiction] = useState(baselines[0]?.jurisdiction ?? '');
  const baseline = baselines.find((pack) => pack.jurisdiction === jurisdiction) ?? baselines[0];
  const set = <K extends keyof ComplianceSettings>(field: K, value: ComplianceSettings[K]) => {
    onChange({ ...compliance, [field]: value });
  };
  return (
    <div className="grid max-w-full gap-8 lg:grid-cols-2">
      <div className="flex flex-col gap-4">
        <SelectField
          label="Jurisdiction"
          value={baseline?.jurisdiction ?? ''}
          onValueChange={setJurisdiction}
          options={baselines.map((pack) => ({
            value: pack.jurisdiction,
            label: pack.jurisdiction,
          }))}
          description="Each agent's jurisdiction picks its pack; see each one here."
        />
        {baseline !== undefined && <PackBaseline baseline={baseline} />}
      </div>
      <div className="flex flex-col gap-6">
        <TextListField
          label="Extra opt-out phrases"
          noun="opt-out phrase"
          items={compliance.optOutPhrases}
          onItemsChange={(items) => {
            set('optOutPhrases', items);
          }}
          maxItems={LIMITS.phrases}
          readOnly={readOnly}
          maxLength={LIMITS.phrase}
          empty="Only the pack's."
        />
        <TextListField
          label="Extra forbidden claims"
          noun="forbidden claim"
          items={compliance.forbiddenClaims}
          onItemsChange={(items) => {
            set('forbiddenClaims', items);
          }}
          maxItems={LIMITS.claims}
          readOnly={readOnly}
          maxLength={LIMITS.item}
          empty="Only the pack's."
        />
        <RetentionCaps compliance={compliance} onChange={onChange} readOnly={readOnly} />
      </div>
    </div>
  );
}
