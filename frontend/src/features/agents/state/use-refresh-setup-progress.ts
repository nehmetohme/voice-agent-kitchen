import { useQueryClient } from '@tanstack/react-query';
import { useCallback } from 'react';

import { agentKeys } from './agents-queries';

/**
 * A callback that refetches what another step's save can change: the agent's setup progress,
 * its draft (a new draft hash) and the list's progress. Routes pass it to the Plan step.
 */
export function useRefreshSetupProgress(agentId: string): () => void {
  const queryClient = useQueryClient();
  return useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: agentKeys.progress(agentId) });
    void queryClient.invalidateQueries({ queryKey: agentKeys.detail(agentId) });
    void queryClient.invalidateQueries({ queryKey: agentKeys.list() });
  }, [agentId, queryClient]);
}
