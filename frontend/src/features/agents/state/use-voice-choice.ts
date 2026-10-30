import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';

import type { Choice } from '../api/agents-api';
import { voicesQueryOptions } from './agents-queries';

export interface VoiceChoice {
  /** The chosen voice, or the recommended (first) one until the operator picks another. */
  value: string;
  choose: (voice: string) => void;
  options: readonly Choice[];
  loading: boolean;
  failed: boolean;
}

/** The voice picker's state: the catalog, with the recommended voice preselected (Tesler). */
export function useVoiceChoice(): VoiceChoice {
  const voices = useQuery(voicesQueryOptions());
  const [chosen, setChosen] = useState('');
  const options = voices.data ?? [];
  return {
    value: chosen || (options[0]?.value ?? ''),
    choose: setChosen,
    options,
    loading: voices.isPending,
    failed: voices.isError,
  };
}
