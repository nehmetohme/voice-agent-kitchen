/**
 * The setup checklist (Zeigarnik, ZEIG-1..3): four steps in flow order, each done, next or
 * still to do. Pure; the progress itself comes from the server.
 */
import type { SetupProgress, SetupStep } from '../api/agents-api';

export type StepState = 'done' | 'next' | 'todo';

interface ChecklistStep {
  step: SetupStep;
  label: string;
  /** What the step means, in one line. */
  hint: string;
  state: StepState;
}

export interface Checklist {
  percent: number;
  steps: ChecklistStep[];
}

const STEPS: Record<SetupStep, { label: string; hint: string }> = {
  provision: { label: 'Provision', hint: 'Name, purpose and voice' },
  plan: { label: 'Plan', hint: 'Goals, flow and guardrails' },
  // Talking to the agent already works on the Provision page; the Test step adds scenarios (M5).
  test: { label: 'Test', hint: 'Run test scenarios and scores' },
  publish: { label: 'Publish', hint: 'Release a version to a channel' },
};

/** Labels each step and marks the first unfinished one as next. */
export function toChecklist(progress: SetupProgress): Checklist {
  return {
    percent: progress.percent,
    steps: progress.steps.map(({ step, done }) => {
      const next = !done && step === progress.nextStep;
      return { step, ...STEPS[step], state: done ? 'done' : next ? 'next' : 'todo' };
    }),
  };
}
