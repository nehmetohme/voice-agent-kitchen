// What the Test step shows about a run, in words: pure, so every rule is unit-tested.
import type {
  RubricCheck,
  ScenarioResult,
  ScenarioStatus,
  TestRunView,
} from '../api/test-runs-api';

/** How a scenario stands in the latest run; `unrun` when the run doesn't have it. */
export type Standing = ScenarioStatus | 'unrun';

export const STANDING_LABEL: Record<Standing, string> = {
  pending: 'Waiting',
  running: 'Running',
  passed: 'Passed',
  failed: 'Failed',
  error: 'Error',
  skipped: 'Skipped',
  unrun: 'Not run',
};

/** The run's state in one sentence: outcome first (SP-4). */
export function runStatusText(run: TestRunView | null): string {
  if (run === null) return 'No test run yet.';
  const { done, total } = run.progress;
  const required = run.scenarios.filter((result) => result.required);
  const passed = required.filter((result) => result.status === 'passed').length;
  const tally = `${passed} of ${required.length} required ${required.length === 1 ? 'scenario' : 'scenarios'} passed.`;
  switch (run.status) {
    case 'queued':
      return 'Queued: waiting to start.';
    case 'running':
      return `Running: ${done} of ${total} scenarios done.`;
    case 'cancelled':
      return 'Cancelled.';
    case 'interrupted':
      return 'Interrupted: the server stopped during the run.';
    case 'finished':
    default:
      return run.verdict === 'passed' ? `Passed: ${tally}` : `Failed: ${tally}`;
  }
}

/** The scenario's result in `run`, if the run has it. */
export function resultOf(run: TestRunView | null, scenarioId: string): ScenarioResult | null {
  return run?.scenarios.find((result) => result.id === scenarioId) ?? null;
}

export function standingOf(run: TestRunView | null, scenarioId: string): Standing {
  return resultOf(run, scenarioId)?.status ?? 'unrun';
}

const LAYER: Record<NonNullable<ScenarioResult['optOutLayer']>, string> = {
  phrase: 'an opt-out phrase',
  decision: 'the decision model',
  llm_tool: 'the agent',
};

/** How the conversation ended, or what is still to come. */
export function endingText(result: ScenarioResult): string {
  switch (result.endReason) {
    case 'caller_hung_up':
      return 'The caller hung up.';
    case 'opt_out':
      return result.optOutLayer === null
        ? 'The caller opted out, so the agent ended the call.'
        : `The caller opted out, caught by ${LAYER[result.optOutLayer]}, so the agent ended the call.`;
    case 'turn_cap':
      return 'The conversation reached its turn cap.';
    case 'error':
      return "A model didn't answer, so this scenario couldn't be judged.";
    case null:
    default:
      return result.status === 'skipped' ? 'Skipped: the run stopped first.' : 'Not played yet.';
  }
}

/** "3.2 s", or "850 ms" under a second. */
export function durationText(ms: number | null): string | null {
  if (ms === null) return null;
  return ms < 1_000 ? `${ms} ms` : `${(ms / 1_000).toFixed(1)} s`;
}

/** What a check found: the expected answer and the model's, with its probability of yes. */
export function checkText(check: RubricCheck): string {
  const expected = `Expected ${check.expected}`;
  if (check.probability === null) return `${expected} · no answer`;
  const answer = check.probability >= 0.5 ? 'yes' : 'no';
  return `${expected} · answered ${answer} (${check.probability.toFixed(2)})`;
}

/** "fake-llm · fake-decisions · seed 1": what the run used, for reproducing it. */
export function modelsText(run: TestRunView | null): string | null {
  if (run === null || run.models === null) return null;
  const { llm, decisions, seed } = run.models;
  return `${llm} · ${decisions} · seed ${seed}`;
}
