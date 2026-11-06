import { useQuery } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';

import type { Scenario } from '../api/test-runs-api';
import { setProblem, withoutScenario, withScenario } from './scenario-edits';
import {
  isActive,
  scenariosQueryOptions,
  testRunQueryOptions,
  testRunsQueryOptions,
  useRunControls,
  useSaveScenarios,
} from './test-runs-queries';

/** The agent's latest run, read again while it runs; `onEnded` runs when it stops. */
function useLatestRun(agentId: string, onEnded?: () => void) {
  const runs = useQuery(testRunsQueryOptions(agentId));
  const latest = runs.data?.[0] ?? null;
  const run = useQuery({ ...testRunQueryOptions(latest?.id ?? ''), enabled: latest !== null });
  const wasActive = useRef(false);
  const status = run.data?.status;
  useEffect(() => {
    const active = status !== undefined && isActive(status);
    if (wasActive.current && !active) onEnded?.();
    wasActive.current = active;
  }, [status, onEnded]);
  return {
    /** `null` when the agent has no run (or it is still loading). */
    run: run.data ?? null,
    /** The latest run tested an earlier draft. */
    stale: latest !== null && !latest.current,
    loading: runs.isPending || (latest !== null && run.isPending),
  };
}

/** The agent's scenarios, which one is shown, and saving changes to the set. */
function useScenarioSet(agentId: string, onSaved?: () => void) {
  const scenarios = useQuery(scenariosQueryOptions(agentId));
  const save = useSaveScenarios(agentId, onSaved);
  const [chosen, setChosen] = useState<string | null>(null);
  const [problem, setLocalProblem] = useState<string | null>(null);
  const set = scenarios.data?.scenarios ?? [];
  const saveSet = (next: Scenario[]) => {
    const why = setProblem(next);
    setLocalProblem(why);
    if (why === null) save.mutate(next);
  };
  const failed = save.isError ? "Couldn't save the scenarios. Try again." : null;
  return {
    scenarios,
    selected: set.find((scenario) => scenario.id === chosen) ?? set[0] ?? null,
    select: setChosen,
    saveScenario: (scenario: Scenario) => {
      saveSet(withScenario(set, scenario));
    },
    deleteScenario: (id: string) => {
      saveSet(withoutScenario(set, id));
    },
    /** Why the last change to the set couldn't be saved, if it couldn't. */
    setProblem: problem ?? failed,
    saving: save.isPending,
  };
}

/**
 * The Test step of one agent: its scenarios, its latest run, which scenario is shown, and the
 * actions. `onChange` runs after the draft or the Test step may have changed: a saved set, or
 * a run that ended.
 */
export function useTestPage(agentId: string, onChange?: () => void) {
  return {
    ...useScenarioSet(agentId, onChange),
    latest: useLatestRun(agentId, onChange),
    ...useRunControls(agentId),
  };
}

export type TestPageModel = ReturnType<typeof useTestPage>;
