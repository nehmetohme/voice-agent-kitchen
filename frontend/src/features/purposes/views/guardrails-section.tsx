import { Lock, X } from 'lucide-react';

import { Button } from '../../../shared/ui/button';
import { CheckboxField } from '../../../shared/ui/checkbox-field';
import { TextField } from '../../../shared/ui/text-field';
import { TextListField } from '../../../shared/ui/text-list-field';
import { PERSONAL_DATA, actionLabel } from '../state/choices';
import { LIMITS, type GuardrailSettings } from '../state/models';
import { EscalationDialog } from './escalation-dialog';

interface GuardrailsSectionProps {
  guardrails: GuardrailSettings;
  onChange: (guardrails: GuardrailSettings) => void;
  /** The rule pack's forbidden claims (default jurisdiction), shown locked beside "Never". */
  packClaims: readonly string[];
  readOnly: boolean;
}

/**
 * What the agent may talk about, what it must never say or do, when to escalate, and its
 * limits. Test calls already follow the topics, the never-rules and the never-ask list (they
 * are in the instructions); the decision engine enforces them, and acts on escalations, in M4b.
 */
export function GuardrailsSection({
  guardrails,
  onChange,
  packClaims,
  readOnly,
}: GuardrailsSectionProps) {
  const set = <K extends keyof GuardrailSettings>(field: K, value: GuardrailSettings[K]) => {
    onChange({ ...guardrails, [field]: value });
  };
  return (
    <div className="grid max-w-full gap-8 lg:grid-cols-2">
      <div className="flex flex-col gap-6">
        <TextListField
          label="Allowed topics"
          noun="topic"
          items={guardrails.allowedTopics}
          onItemsChange={(items) => {
            set('allowedTopics', items);
          }}
          description="The agent stays on these and steers other requests back. None means any topic."
          maxItems={LIMITS.topics}
          readOnly={readOnly}
          maxLength={LIMITS.item}
          empty="Any topic."
        />
        <TextListField
          label="Never"
          noun="never-rule"
          items={guardrails.never}
          onItemsChange={(items) => {
            set('never', items);
          }}
          description="Things the agent must never say or do, such as promise refunds."
          maxItems={LIMITS.never}
          readOnly={readOnly}
          maxLength={LIMITS.item}
          empty="Nothing beyond the rule pack."
        />
        <section aria-label="From the rule pack" className="flex flex-col gap-1">
          <p className="flex items-center gap-2 text-sm font-medium text-muted">
            <Lock aria-hidden className="size-4" />
            Always never, from the rule pack
          </p>
          <ul aria-label="From the rule pack" className="flex flex-col gap-1 text-sm text-muted">
            {packClaims.map((claim) => (
              <li key={claim}>{claim}</li>
            ))}
          </ul>
        </section>
      </div>
      <div className="flex flex-col gap-6">
        <section aria-label="Escalation rules" className="flex flex-col gap-2">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <h3 className="text-sm font-medium">Escalation rules</h3>
            {!readOnly && guardrails.escalations.length < LIMITS.escalations && (
              <EscalationDialog
                onAdd={(rule) => {
                  set('escalations', [...guardrails.escalations, rule]);
                }}
              />
            )}
          </div>
          {guardrails.escalations.length === 0 && (
            <p className="text-sm text-muted">No escalations yet.</p>
          )}
          <ul aria-label="Escalation rules" className="flex flex-col gap-1">
            {guardrails.escalations.map((rule) => (
              <li
                key={rule.when}
                className="flex items-center justify-between gap-2 rounded-md bg-surface pl-3"
              >
                <span className="text-sm">{`${rule.when} → ${actionLabel(rule.action)}`}</span>
                {!readOnly && (
                  <Button
                    variant="ghost"
                    aria-label={`Remove ${rule.when}`}
                    onClick={() => {
                      set(
                        'escalations',
                        guardrails.escalations.filter((other) => other !== rule),
                      );
                    }}
                  >
                    <X aria-hidden className="size-4" />
                  </Button>
                )}
              </li>
            ))}
          </ul>
        </section>
        <fieldset disabled={readOnly} className="flex flex-col gap-2">
          <legend className="text-sm font-medium">Limits</legend>
          <TextField
            label="Longest call (minutes)"
            type="number"
            min={1}
            max={LIMITS.callMinutes}
            value={String(guardrails.maxCallMinutes)}
            onValueChange={(value) => {
              set('maxCallMinutes', Number(value));
            }}
          />
          <fieldset className="flex flex-col">
            <legend className="text-sm text-muted">Never ask the caller for</legend>
            {PERSONAL_DATA.map(({ value, label }) => (
              <CheckboxField
                key={value}
                label={label}
                checked={guardrails.neverAsk.includes(value)}
                onCheckedChange={(on) => {
                  const others = guardrails.neverAsk.filter((kind) => kind !== value);
                  set('neverAsk', on ? [...others, value] : others);
                }}
              />
            ))}
          </fieldset>
        </fieldset>
      </div>
    </div>
  );
}
