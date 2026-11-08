import { useQuery } from '@tanstack/react-query';

import type { PurposeChoice } from '../api/purpose-api';
import { purposeChoicesQueryOptions } from './agents-queries';
import { BUILT_IN_PURPOSES } from './choices';

/**
 * The purposes an agent can pin: the built-in ones at once, then every purpose with a saved
 * version once the list loads. `current` stays listed even if the list no longer has it.
 */
export function usePurposeChoices(current?: { value: string; label: string }): PurposeChoice[] {
  const purposes = useQuery(purposeChoicesQueryOptions());
  const choices = purposes.data ?? [...BUILT_IN_PURPOSES];
  if (current === undefined || choices.some((choice) => choice.value === current.value)) {
    return choices;
  }
  const kept: PurposeChoice = { ...current, useCase: 'custom', system: false };
  return [...choices, kept];
}
