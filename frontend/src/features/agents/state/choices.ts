/** The fixed choices the agent forms offer: six purposes and two model profiles (Hick). */
import type { Choice, ModelProfile, UseCase } from '../api/agents-api';

/**
 * The catalog's choices, always including `current`: a picker must show the stored value even
 * before the catalog loads, or if the catalog no longer lists it.
 */
export function withCurrent(options: readonly Choice[] | undefined, current: string): Choice[] {
  const list = [...(options ?? [])];
  return list.some((option) => option.value === current)
    ? list
    : [{ value: current, label: current }, ...list];
}

export interface PurposeChoice {
  value: UseCase;
  label: string;
}

/** Every purpose, in the order the server lists use cases. */
export const PURPOSES: readonly PurposeChoice[] = [
  { value: 'support', label: 'Customer support' },
  { value: 'service', label: 'Customer service' },
  { value: 'booking', label: 'Appointment booking' },
  { value: 'survey', label: 'Survey' },
  { value: 'telemarketing', label: 'Outbound sales' },
  { value: 'custom', label: 'Something else' },
];

export const MODELS: readonly { value: ModelProfile; label: string }[] = [
  { value: 'balanced', label: 'Balanced (recommended)' },
  { value: 'fast', label: 'Fast: quicker replies, simpler answers' },
];

/** The purpose's label, e.g. `Customer support`. */
export function purposeLabel(useCase: UseCase): string {
  return PURPOSES.find((purpose) => purpose.value === useCase)?.label ?? useCase;
}

/** Narrows a select's value back to a purpose. */
export function toUseCase(value: string): UseCase | undefined {
  return PURPOSES.find((purpose) => purpose.value === value)?.value;
}

/** Narrows a select's value back to a model profile. */
export function toModel(value: string): ModelProfile | undefined {
  return MODELS.find((model) => model.value === value)?.value;
}
