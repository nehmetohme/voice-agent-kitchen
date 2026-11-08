import { useMutation, useQueryClient } from '@tanstack/react-query';

import {
  archiveAgent,
  createAgent,
  updateAgent,
  type AgentDetails,
  type AgentSettings,
  type NewAgent,
} from '../api/agents-api';
import { updateAgentPurpose, type AgentPurpose } from '../api/purpose-api';
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
      void queryClient.invalidateQueries({ queryKey: agentKeys.purpose(id) });
      void queryClient.invalidateQueries({ queryKey: agentKeys.list() });
    },
  });
}

/**
 * Moves the agent to another version of its purpose. Its settings and progress follow, so they
 * are read again; the Plan step reads its plan whenever it opens.
 */
export function useUpdateAgentPurpose(id: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ version, replacePlan }: { version: number; replacePlan: boolean }) =>
      updateAgentPurpose(id, version, replacePlan),
    onSuccess: (purpose: AgentPurpose) => {
      queryClient.setQueryData(agentKeys.purpose(id), purpose);
      void queryClient.invalidateQueries({ queryKey: agentKeys.detail(id) });
      void queryClient.invalidateQueries({ queryKey: agentKeys.progress(id) });
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
