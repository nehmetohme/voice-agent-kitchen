import { Link, useNavigate } from '@tanstack/react-router';
import { useState, type SubmitEvent } from 'react';

import { Button, buttonLook } from '../../../shared/ui/button';
import { ChoiceField } from '../../../shared/ui/choice-field';
import { SelectField } from '../../../shared/ui/select-field';
import { TextField } from '../../../shared/ui/text-field';
import { useCreateAgent } from '../state/agent-mutations';
import { LIMITS, fieldErrorsOf, type FieldErrors } from '../state/models';
import { checkNewAgent } from '../state/new-agent';
import { usePurposeChoices } from '../state/use-purpose-choices';
import { useVoiceChoice } from '../state/use-voice-choice';

/**
 * Create an agent with only the three things it needs (Tesler, TES-1): a name, a purpose and
 * a voice. The recommended voice is preselected, and the purpose too when the page opens from
 * one (`initialPurpose`); everything else has a server default. The purpose list grows with
 * the workspace's purposes: past seven it is a searchable combobox (HICK-3).
 */
export function CreateAgentPage({ initialPurpose }: { initialPurpose?: string | undefined }) {
  const navigate = useNavigate();
  const voice = useVoiceChoice();
  const create = useCreateAgent();
  const purposes = usePurposeChoices();
  const [name, setName] = useState('');
  const [purpose, setPurpose] = useState(initialPurpose ?? '');
  const [errors, setErrors] = useState<FieldErrors>({});
  const failedWithoutDetail = create.isError && Object.keys(errors).length === 0;

  const submit = (event: SubmitEvent<HTMLFormElement>) => {
    event.preventDefault();
    const check = checkNewAgent({ name, purpose, voice: voice.value }, purposes);
    setErrors(check.errors ?? {});
    if (check.agent === undefined) return;
    create.mutate(check.agent, {
      onSuccess: (agent) => {
        void navigate({ to: '/agents/$agentId/provision', params: { agentId: agent.id } });
      },
      onError: (error) => {
        setErrors(fieldErrorsOf(error));
      },
    });
  };

  return (
    <form
      aria-label="Create agent"
      noValidate
      onSubmit={submit}
      className="flex max-w-readable flex-col gap-6"
    >
      <p className="text-muted">
        Three things to start. Language, compliance rules, model and timing get sensible defaults
        that you can change on the next page.
      </p>
      <div className="flex flex-col gap-4">
        <TextField
          label="Name"
          value={name}
          onValueChange={setName}
          error={errors.name}
          maxLength={LIMITS.nameMax}
          autoComplete="off"
          required
        />
        <ChoiceField
          label="Purpose"
          value={purpose}
          onValueChange={setPurpose}
          options={purposes}
          placeholder="Choose a purpose"
          error={errors.purpose}
        />
        <SelectField
          label="Voice"
          value={voice.value}
          onValueChange={voice.choose}
          options={voice.options}
          disabled={voice.loading}
          description={voice.loading ? 'Loading voices…' : 'You can hear it in a test call later.'}
          error={errors.voice ?? (voice.failed ? 'Voices could not be loaded.' : undefined)}
        />
      </div>
      {failedWithoutDetail && (
        <p role="alert" className="text-danger">
          The agent wasn't created. Check your connection, then try again.
        </p>
      )}
      <div data-region="actions" className="flex gap-2">
        <Button
          variant="primary"
          type="submit"
          disabled={create.isPending}
          aria-busy={create.isPending}
        >
          Create agent
        </Button>
        <Link to="/agents" {...buttonLook()}>
          Cancel
        </Link>
      </div>
    </form>
  );
}
