import { useMutation, useQueryClient } from '@tanstack/react-query';

import {
  archiveAgent,
  createAgent,
  updateAgent,
  type AgentDetails,
  type AgentSettings,
  type NewAgent,
} from '../api/agents-api';
import { agentKeys } from './agents-queries';

/** Creates an agent and seeds its detail query, so its page opens without a fetch. */
export function useCreateAgent() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (agent: NewAgent) => createAgent(agent),
    onSuccess: (created: AgentDetails) => {
      queryClient.setQueryData(agentKeys.detail(created.id), created);
      void queryClient.invalidateQueries({ queryKey: agentKeys.list() });
    },
  });
}

/** Saves draft changes; the saved agent replaces the cached one. */
export function useUpdateAgent(id: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (changes: Partial<AgentSettings>) => updateAgent(id, changes),
    onSuccess: (saved: AgentDetails) => {
      queryClient.setQueryData(agentKeys.detail(id), saved);
      void queryClient.invalidateQueries({ queryKey: agentKeys.progress(id) });
      void queryClient.invalidateQueries({ queryKey: agentKeys.list() });
    },
  });
}

/** Archives an agent and drops it from the list. */
export function useArchiveAgent(id: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => archiveAgent(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: agentKeys.all });
    },
  });
}
