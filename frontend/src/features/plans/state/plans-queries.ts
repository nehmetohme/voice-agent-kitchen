import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query';

import { getPlan, savePlan, type PlanDraft, type PlanView } from '../api/plans-api';

/** Query keys: one agent's plan. */
const planKeys = {
  all: ['plans'] as const,
  detail: (agentId: string) => [...planKeys.all, agentId] as const,
};

/** The agent's plan, or its template until one is saved. */
export const planQueryOptions = (agentId: string) =>
  queryOptions({ queryKey: planKeys.detail(agentId), queryFn: () => getPlan(agentId) });

/** Saves the plan; the server's answer (with its problems) replaces the cached plan. */
export function useSavePlan(agentId: string, onSaved?: () => void) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (draft: PlanDraft) => savePlan(agentId, draft),
    onSuccess: (saved: PlanView) => {
      queryClient.setQueryData(planKeys.detail(agentId), saved);
      onSaved?.();
    },
  });
}
