/**
 * The contract's request rules, taken from the generated schemas so the page can't drift from
 * the server: the limits forms show, and the check autosave runs before it sends anything.
 * Rules the schema doesn't carry (trimming, region codes) are the server's: its 422 names the
 * field.
 */
import {
  UpdateAgentBody,
  updateAgentBodyNameMax,
  updateAgentBodyTurnDetectionSilenceMsMax,
  updateAgentBodyTurnDetectionSilenceMsMin,
} from '../../../shared/api/generated/model';
import type { AgentSettings, FieldErrors } from './agents-api';

export const LIMITS = {
  nameMax: updateAgentBodyNameMax,
  silenceMinMs: updateAgentBodyTurnDetectionSilenceMsMin,
  silenceMaxMs: updateAgentBodyTurnDetectionSilenceMsMax,
} as const;

const MESSAGES: Record<keyof AgentSettings, string> = {
  name: `Enter a name of up to ${LIMITS.nameMax} characters.`,
  useCase: 'Choose a purpose.',
  voice: 'Choose a voice.',
  language: 'Choose a language.',
  jurisdiction: 'Enter a region code.',
  model: 'Choose a model.',
  turnDetection: `Use a pause between ${LIMITS.silenceMinMs} and ${LIMITS.silenceMaxMs} ms.`,
};

function isField(field: PropertyKey | undefined): field is keyof AgentSettings {
  return typeof field === 'string' && Object.hasOwn(MESSAGES, field);
}

/** The fields of `settings` that break the contract's request schema, with what to fix. */
export function checkSettings(settings: Partial<AgentSettings>): FieldErrors {
  const result = UpdateAgentBody.safeParse(settings);
  const errors: FieldErrors = {};
  for (const issue of result.success ? [] : result.error.issues) {
    const field = issue.path[0];
    if (isField(field)) errors[field] = MESSAGES[field];
  }
  return errors;
}
