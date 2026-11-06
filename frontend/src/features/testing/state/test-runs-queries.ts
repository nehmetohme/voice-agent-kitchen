import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query';

import {
  cancelTestRun,
  getScenarios,
  getTestRun,
  listTestRuns,
  saveScenarios,
  startTestRun,
  type Scenario,
  type ScenarioSetView,
  type TestRunStatus,
  type TestRunView,
} from '../api/test-runs-api';

/** How often a queued or running run is read again. */
const RUN_POLL_MS = 1_000;

/** Query keys: one agent's scenarios and runs, and one run. */
const testRunKeys = {
  all: ['test-runs'] as const,
  scenarios: (agentId: string) => [...testRunKeys.all, 'scenarios', agentId] as const,
  list: (agentId: string) => [...testRunKeys.all, 'list', agentId] as const,
  run: (runId: string) => [...testRunKeys.all, 'run', runId] as const,
};

/** Queued or running: its results are still coming. */
export const isActive = (status: TestRunStatus) => status === 'queued' || status === 'running';

/** The agent's saved scenarios, or its use case's defaults. */
export const scenariosQueryOptions = (agentId: string) =>
  queryOptions({
    queryKey: testRunKeys.scenarios(agentId),
    queryFn: () => getScenarios(agentId),
  });

/** The agent's runs, newest first, each marked current or stale. */
export const testRunsQueryOptions = (agentId: string) =>
  queryOptions({ queryKey: testRunKeys.list(agentId), queryFn: () => listTestRuns(agentId) });

/** One run, read again every second while it is queued or running. */
export const testRunQueryOptions = (runId: string) =>
  queryOptions({
    queryKey: testRunKeys.run(runId),
    queryFn: () => getTestRun(runId),
    refetchInterval: (query) => {
      const status = query.state.data?.status;
      return status !== undefined && isActive(status) ? RUN_POLL_MS : false;
    },
  });

/** Saves the whole set; the server's answer replaces the cached scenarios. */
export function useSaveScenarios(agentId: string, onSaved?: () => void) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (scenarios: readonly Scenario[]) => saveScenarios(agentId, scenarios),
    onSuccess: (saved: ScenarioSetView) => {
      queryClient.setQueryData(testRunKeys.scenarios(agentId), saved);
      // The draft hash changed: the latest run may be stale now.
      void queryClient.invalidateQueries({ queryKey: testRunKeys.list(agentId) });
      onSaved?.();
    },
  });
}

/** Starts and cancels runs; each answer is cached as the run, and the list is read again. */
export function useRunControls(agentId: string) {
  const queryClient = useQueryClient();
  const cache = (run: TestRunView) => {
    queryClient.setQueryData(testRunKeys.run(run.id), run);
    void queryClient.invalidateQueries({ queryKey: testRunKeys.list(agentId) });
  };
  const start = useMutation({ mutationFn: () => startTestRun(agentId), onSuccess: cache });
  const cancel = useMutation({ mutationFn: cancelTestRun, onSuccess: cache });
  return { start, cancel };
}
