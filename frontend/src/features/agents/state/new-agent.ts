/** The create form's rules: the three required fields, checked before anything is sent. */
import type { FieldErrors, NewAgent } from '../api/agents-api';
import { toUseCase } from './choices';

export interface NewAgentForm {
  name: string;
  purpose: string;
  voice: string;
}

export type NewAgentCheck =
  | { agent: NewAgent; errors?: never }
  | { agent?: never; errors: FieldErrors };

/** The agent to create, or what to fix first. */
export function checkNewAgent(form: NewAgentForm): NewAgentCheck {
  const useCase = toUseCase(form.purpose);
  const errors: FieldErrors = {};
  if (form.name.trim() === '') errors.name = 'Enter a name.';
  if (useCase === undefined) errors.useCase = 'Choose a purpose.';
  if (form.voice === '') errors.voice = 'Choose a voice.';
  if (useCase === undefined || Object.keys(errors).length > 0) return { errors };
  return { agent: { name: form.name, useCase, voice: form.voice } };
}
