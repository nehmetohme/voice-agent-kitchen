/** The create form's rules: the three required fields, checked before anything is sent. */
import type { FieldErrors, NewAgent } from '../api/agents-api';
import type { PurposeChoice } from '../api/purpose-api';

export interface NewAgentForm {
  name: string;
  /** A purpose picker value: a built-in's use case, or a purpose ID. */
  purpose: string;
  voice: string;
}

export type NewAgentCheck =
  | { agent: NewAgent; errors?: never }
  | { agent?: never; errors: FieldErrors };

/**
 * The agent to create, or what to fix first. A built-in purpose is sent as its use case (the
 * body every client sent before purposes); another one as its ID with its compliance class.
 */
export function checkNewAgent(
  form: NewAgentForm,
  purposes: readonly PurposeChoice[],
): NewAgentCheck {
  const purpose = purposes.find((choice) => choice.value === form.purpose);
  const errors: FieldErrors = {};
  if (form.name.trim() === '') errors.name = 'Enter a name.';
  if (purpose === undefined) errors.purpose = 'Choose a purpose.';
  if (form.voice === '') errors.voice = 'Choose a voice.';
  if (purpose === undefined || Object.keys(errors).length > 0) return { errors };
  const agent: NewAgent = { name: form.name, useCase: purpose.useCase, voice: form.voice };
  if (!purpose.system) agent.purposeId = purpose.value;
  return { agent };
}
