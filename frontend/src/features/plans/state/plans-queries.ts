import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query';

import {
  getFlow,
  saveFlow,
  type PlanDraft,
  type PlanTarget,
  type PlanView,
} from '../api/plans-api';

/** Query keys: one agent's plan, or one purpose's starting flow. */
const planKeys = {
  all: ['plans'] as const,
  detail: (target: PlanTarget) =>
    target.kind === 'agent'
      ? ([...planKeys.all, target.agentId] as const)
      : ([...planKeys.all, 'purpose', target.purposeId] as const),
};

/** The target's flow: an agent's plan (its template until one is saved), or a purpose's. */
export const flowQueryOptions = (target: PlanTarget) =>
  queryOptions({ queryKey: planKeys.detail(target), queryFn: () => getFlow(target) });

/** The agent's plan, or its template until one is saved. */
export const planQueryOptions = (agentId: string) => flowQueryOptions({ kind: 'agent', agentId });

/** Saves the flow; the server's answer (with its problems) replaces the cached one. */
export function useSavePlan(target: PlanTarget, onSaved?: () => void) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (draft: PlanDraft) => saveFlow(target, draft),
    onSuccess: (saved: PlanView) => {
      queryClient.setQueryData(planKeys.detail(target), saved);
      onSaved?.();
    },
  });
}
