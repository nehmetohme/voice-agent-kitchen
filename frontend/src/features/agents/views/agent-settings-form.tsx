import { useQuery } from '@tanstack/react-query';
import { useId } from 'react';

import { SaveStatus } from '../../../shared/ui/save-status';
import { SelectField } from '../../../shared/ui/select-field';
import { TextField } from '../../../shared/ui/text-field';
import { languagesQueryOptions, voicesQueryOptions } from '../state/agents-queries';
import { PURPOSES, toUseCase, withCurrent } from '../state/choices';
import { LIMITS, type AgentDetails } from '../state/models';
import { useAutosave } from '../state/use-autosave';
import { AdvancedSettings } from './advanced-settings';

/**
 * The agent's settings, dense enough to sit beside the test call, saved automatically shortly
 * after each edit (Parkinson): no Save button, and the status beside the title always says
 * whether everything is stored. Fields pair up where they are short; the voice, whose names
 * are long, gets the full width. When "Advanced" opens, the column scrolls, not the page.
 */
export function AgentSettingsForm({ agent }: { agent: AgentDetails }) {
  const { draft, edit, status, errors, retry } = useAutosave(agent);
  const voices = useQuery(voicesQueryOptions());
  const languages = useQuery(languagesQueryOptions());
  const titleId = useId();
  return (
    <section
      aria-labelledby={titleId}
      className="flex flex-col gap-4 rounded-md border border-line p-4 lg:min-h-0 lg:flex-1 lg:overflow-y-auto"
    >
      <div className="flex min-h-target flex-wrap items-center justify-between gap-3">
        <h2 id={titleId} className="text-lg font-semibold">
          Agent settings
        </h2>
        <SaveStatus status={status} onRetry={retry} />
      </div>
      <fieldset className="grid grid-cols-6 items-start gap-x-3 gap-y-4">
        <legend className="sr-only">Who the agent is</legend>
        <div className="col-span-3">
          <TextField
            label="Name"
            value={draft.name}
            onValueChange={(name) => {
              edit('name', name);
            }}
            error={errors.name}
            maxLength={LIMITS.nameMax}
            autoComplete="off"
          />
        </div>
        <div className="col-span-3">
          <SelectField
            label="Purpose"
            value={draft.useCase}
            onValueChange={(value) => {
              const useCase = toUseCase(value);
              if (useCase !== undefined) edit('useCase', useCase);
            }}
            options={PURPOSES}
            description="Also picks the compliance rule pack."
            error={errors.useCase}
          />
        </div>
        <div className="col-span-6">
          <SelectField
            label="Voice"
            value={draft.voice}
            onValueChange={(voice) => {
              edit('voice', voice);
            }}
            options={withCurrent(voices.data, draft.voice)}
            error={errors.voice}
          />
        </div>
        <div className="col-span-4">
          <SelectField
            label="Language"
            value={draft.language}
            onValueChange={(language) => {
              edit('language', language);
            }}
            options={withCurrent(languages.data, draft.language)}
            error={errors.language}
          />
        </div>
        <div className="col-span-2">
          <TextField
            label="Jurisdiction"
            value={draft.jurisdiction}
            onValueChange={(jurisdiction) => {
              edit('jurisdiction', jurisdiction);
            }}
            description="Such as US, US-CA or UK."
            error={errors.jurisdiction}
            maxLength={6}
            autoComplete="off"
          />
        </div>
      </fieldset>
      <AdvancedSettings draft={draft} edit={edit} errors={errors} />
    </section>
  );
}
