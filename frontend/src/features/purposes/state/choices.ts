/** The fixed choices the purpose forms offer, with their labels. */
import type {
  EscalationAction,
  HttpMethod,
  PersonalData,
  ToolInputType,
  UseCase,
} from '../api/purpose-models';

/** Compliance classes: the rule-pack families, named as the built-in purposes are. */
export const CLASSES: readonly { value: UseCase; label: string }[] = [
  { value: 'support', label: 'Customer support' },
  { value: 'service', label: 'Customer service' },
  { value: 'booking', label: 'Appointment booking' },
  { value: 'survey', label: 'Survey' },
  { value: 'telemarketing', label: 'Outbound sales' },
  { value: 'custom', label: 'Something else' },
];

export function classLabel(value: UseCase): string {
  return CLASSES.find((option) => option.value === value)?.label ?? value;
}

export const METHODS: readonly { value: HttpMethod; label: string }[] = [
  { value: 'GET', label: 'GET: read' },
  { value: 'POST', label: 'POST: create' },
  { value: 'PUT', label: 'PUT: replace' },
  { value: 'PATCH', label: 'PATCH: change' },
  { value: 'DELETE', label: 'DELETE: remove' },
];

export const INPUT_TYPES: readonly { value: ToolInputType; label: string }[] = [
  { value: 'string', label: 'Text' },
  { value: 'number', label: 'Number' },
  { value: 'integer', label: 'Whole number' },
  { value: 'boolean', label: 'Yes or no' },
];

export const ACTIONS: readonly { value: EscalationAction; label: string }[] = [
  { value: 'transfer', label: 'Transfer to a person' },
  { value: 'end_call', label: 'End the call with a message' },
  { value: 'flag', label: 'Flag the call for review' },
];

export function actionLabel(value: EscalationAction): string {
  return ACTIONS.find((option) => option.value === value)?.label ?? value;
}

export const PERSONAL_DATA: readonly { value: PersonalData; label: string }[] = [
  { value: 'card_number', label: 'Card numbers' },
  { value: 'national_id', label: 'Social security and ID numbers' },
  { value: 'password', label: 'Passwords and PINs' },
  { value: 'bank_account', label: 'Bank account numbers' },
  { value: 'health', label: 'Health details' },
];

/** Narrows a select's value back to one of `options`. */
export function choose<T extends string>(
  options: readonly { value: T }[],
  value: string,
): T | undefined {
  return options.find((option) => option.value === value)?.value;
}
