/** The choices the agent forms offer: the workspace's purposes and two model profiles (Hick). */
import type { Choice, ModelProfile } from '../api/agents-api';
import type { PurposeChoice } from '../api/purpose-api';

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

/**
 * The six built-in purposes, in the order the server lists them: what the pickers offer
 * before the workspace's purposes load, so creating an agent never waits for them.
 */
export const BUILT_IN_PURPOSES: readonly PurposeChoice[] = [
  { value: 'support', label: 'Customer support', useCase: 'support', system: true },
  { value: 'service', label: 'Customer service', useCase: 'service', system: true },
  { value: 'booking', label: 'Appointment booking', useCase: 'booking', system: true },
  { value: 'survey', label: 'Survey', useCase: 'survey', system: true },
  { value: 'telemarketing', label: 'Outbound sales', useCase: 'telemarketing', system: true },
  { value: 'custom', label: 'Something else', useCase: 'custom', system: true },
];

export const MODELS: readonly { value: ModelProfile; label: string }[] = [
  { value: 'balanced', label: 'Balanced (recommended)' },
  { value: 'fast', label: 'Fast: quicker replies, simpler answers' },
];

/** The purpose's label in `choices`, e.g. `Customer support`, or the value itself. */
export function purposeLabel(choices: readonly PurposeChoice[], value: string): string {
  return choices.find((purpose) => purpose.value === value)?.label ?? value;
}

/** Narrows a select's value back to a model profile. */
export function toModel(value: string): ModelProfile | undefined {
  return MODELS.find((model) => model.value === value)?.value;
}
