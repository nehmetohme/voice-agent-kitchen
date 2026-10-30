import { useQuery } from '@tanstack/react-query';
import { useId } from 'react';

import { SelectField } from '../../../shared/ui/select-field';
import { TextField } from '../../../shared/ui/text-field';
import { languagesQueryOptions, voicesQueryOptions } from '../state/agents-queries';
import { PURPOSES, toUseCase, withCurrent } from '../state/choices';
import type { AgentDetails } from '../state/models';
import { useAutosave } from '../state/use-autosave';
import { AdvancedSettings } from './advanced-settings';
import { SaveStatus } from './save-status';

/**
 * The agent's settings, saved automatically shortly after each edit (Parkinson): no Save
 * button, and the status beside the title always says whether everything is stored.
 */
export function AgentSettingsForm({ agent }: { agent: AgentDetails }) {
  const { draft, edit, status, errors, retry } = useAutosave(agent);
  const voices = useQuery(voicesQueryOptions());
  const languages = useQuery(languagesQueryOptions());
  const titleId = useId();
  return (
    <section
      aria-labelledby={titleId}
      className="flex flex-col gap-6 rounded-md border border-line p-6"
    >
      <div className="flex flex-wrap items-center justify-between gap-4">
        <h2 id={titleId} className="text-lg font-semibold">
          Agent
        </h2>
        <SaveStatus status={status} onRetry={retry} />
      </div>
      <fieldset className="flex flex-col gap-4">
        <legend className="sr-only">Who the agent is</legend>
        <TextField
          label="Name"
          value={draft.name}
          onValueChange={(name) => {
            edit('name', name);
          }}
          error={errors.name}
          maxLength={80}
          autoComplete="off"
        />
        <SelectField
          label="Purpose"
          value={draft.useCase}
          onValueChange={(value) => {
            const useCase = toUseCase(value);
            if (useCase !== undefined) edit('useCase', useCase);
          }}
          options={PURPOSES}
          description="It also picks the compliance rule pack."
          error={errors.useCase}
        />
        <SelectField
          label="Voice"
          value={draft.voice}
          onValueChange={(voice) => {
            edit('voice', voice);
          }}
          options={withCurrent(voices.data, draft.voice)}
          error={errors.voice}
        />
        <SelectField
          label="Language"
          value={draft.language}
          onValueChange={(language) => {
            edit('language', language);
          }}
          options={withCurrent(languages.data, draft.language)}
          error={errors.language}
        />
        <TextField
          label="Jurisdiction"
          value={draft.jurisdiction}
          onValueChange={(jurisdiction) => {
            edit('jurisdiction', jurisdiction);
          }}
          description="Where calls happen, such as US, US-CA or UK. It picks the rule pack's variant."
          error={errors.jurisdiction}
          maxLength={6}
          autoComplete="off"
        />
      </fieldset>
      <AdvancedSettings draft={draft} edit={edit} errors={errors} />
    </section>
  );
}
