// The Test step's pure rules: what a run says, editing the scenario set, and a call's summary.
import { describe, expect, it } from 'vitest';

import type { ScenarioResult, TestRunView } from '../api/test-runs-api';
import { callSummaryOf, percentile } from './call-summary';
import {
  checkText,
  durationText,
  endingText,
  modelsText,
  resultOf,
  runStatusText,
  standingOf,
} from './run-view';
import {
  formOf,
  newScenarioForm,
  newScenarioId,
  scenarioFromForm,
  setProblem,
  withCheck,
  withoutCheck,
  withoutScenario,
  withScenario,
} from './scenario-edits';
import { initialCall, type TestCallState, type TranscriptLine } from './test-call';
import { isActive } from './test-runs-queries';

function result(overrides: Partial<ScenarioResult> = {}): ScenarioResult {
  return {
    id: 'a',
    name: 'A',
    required: true,
    status: 'passed',
    endReason: 'caller_hung_up',
    optOutLayer: null,
    durationMs: 3_200,
    transcript: [],
    checks: [],
    ...overrides,
  };
}

function run(overrides: Partial<TestRunView> = {}): TestRunView {
  return {
    id: 'run',
    status: 'finished',
    verdict: 'passed',
    draftHash: 'sha256:x',
    progress: { done: 2, total: 2, percent: 100 },
    models: { llm: 'KAT-Coder', decisions: 'Kev-4B', seed: 1 },
    scenarios: [result(), result({ id: 'b', required: false, status: 'failed' })],
    ...overrides,
  };
}

const scenario = {
  id: 'books',
  name: 'Books',
  goal: 'Book a table.',
  persona: '',
  required: true,
  maxTurns: 6,
  rubric: [{ question: 'Booked?', expected: 'yes' as const }],
};

describe('a run in words', () => {
  it('opens with the outcome', () => {
    expect(runStatusText(null)).toBe('No test run yet.');
    expect(runStatusText(run())).toBe('Passed: 1 of 1 required scenario passed.');
    const failed = run({
      verdict: 'failed',
      scenarios: [result(), result({ id: 'c', status: 'failed' })],
    });
    expect(runStatusText(failed)).toBe('Failed: 1 of 2 required scenarios passed.');
    expect(runStatusText(run({ status: 'queued' }))).toBe('Queued: waiting to start.');
    expect(
      runStatusText(run({ status: 'running', progress: { done: 1, total: 3, percent: 33 } })),
    ).toBe('Running: 1 of 3 scenarios done.');
    expect(runStatusText(run({ status: 'cancelled' }))).toBe('Cancelled.');
    expect(runStatusText(run({ status: 'interrupted' }))).toMatch(/^Interrupted/);
  });

  it('finds each scenario in the run', () => {
    expect(resultOf(run(), 'b')?.status).toBe('failed');
    expect(resultOf(null, 'b')).toBeNull();
    expect(standingOf(run(), 'b')).toBe('failed');
    expect(standingOf(run(), 'gone')).toBe('unrun');
  });

  it('says how each conversation ended', () => {
    expect(endingText(result())).toBe('The caller hung up.');
    expect(endingText(result({ endReason: 'opt_out' }))).toBe(
      'The caller opted out, so the agent ended the call.',
    );
    expect(endingText(result({ endReason: 'opt_out', optOutLayer: 'phrase' }))).toContain(
      'an opt-out phrase',
    );
    expect(endingText(result({ endReason: 'opt_out', optOutLayer: 'decision' }))).toContain(
      'the decision model',
    );
    expect(endingText(result({ endReason: 'opt_out', optOutLayer: 'llm_tool' }))).toContain(
      'the agent',
    );
    expect(endingText(result({ endReason: 'turn_cap' }))).toContain('turn cap');
    expect(endingText(result({ endReason: 'error' }))).toContain("couldn't be judged");
    expect(endingText(result({ endReason: null, status: 'skipped' }))).toMatch(/^Skipped/);
    expect(endingText(result({ endReason: null, status: 'pending' }))).toBe('Not played yet.');
  });

  it('shows durations, checks and models briefly', () => {
    expect(durationText(null)).toBeNull();
    expect(durationText(850)).toBe('850 ms');
    expect(durationText(3_240)).toBe('3.2 s');
    const check = { question: 'Q?', expected: 'no' as const, passed: true };
    expect(checkText({ ...check, probability: 0.1 })).toBe('Expected no · answered no (0.10)');
    expect(checkText({ ...check, probability: 0.5 })).toBe('Expected no · answered yes (0.50)');
    expect(checkText({ ...check, probability: null })).toBe('Expected no · no answer');
    expect(modelsText(run())).toBe('KAT-Coder · Kev-4B · seed 1');
    expect(modelsText(run({ models: null }))).toBeNull();
    expect(modelsText(null)).toBeNull();
    expect([isActive('queued'), isActive('running'), isActive('finished')]).toEqual([
      true,
      true,
      false,
    ]);
  });
});

describe('editing scenarios', () => {
  it('starts a new scenario required only when nothing else is', () => {
    expect(newScenarioForm([]).required).toBe(true);
    expect(newScenarioForm([scenario]).required).toBe(false);
    expect(newScenarioForm([]).checks).toEqual([{ question: '', expected: 'yes' }]);
  });

  it('builds a scenario from a valid form, trimmed', () => {
    const form = { ...formOf(scenario), name: '  Books  ', maxTurns: '4' };
    expect(scenarioFromForm(form, 'books')).toEqual({
      ok: true,
      scenario: { ...scenario, maxTurns: 4 },
    });
  });

  it('names every problem of an invalid form', () => {
    const form = {
      ...newScenarioForm([]),
      persona: 'p'.repeat(1_001),
      maxTurns: '0',
      checks: [
        { question: 'Fine?', expected: 'yes' as const },
        { question: '', expected: 'no' as const },
      ],
    };
    const outcome = scenarioFromForm(form, 'x');
    expect(outcome).toEqual({
      ok: false,
      errors: {
        name: 'Write the name.',
        goal: "Write the caller's goal.",
        persona: 'Keep the persona to 1000 characters.',
        maxTurns: 'Allow 1 to 20 caller turns.',
        checks: [undefined, 'Write the question.'],
      },
    });
    const long = scenarioFromForm({ ...formOf(scenario), name: 'n'.repeat(81) }, 'x');
    expect(long).toMatchObject({ ok: false, errors: { name: 'Keep the name to 80 characters.' } });
  });

  it('adds and removes checks within the limits', () => {
    let form = formOf(scenario);
    expect(withoutCheck(form, 0)).toBe(form);
    for (let i = 0; i < 10; i += 1) form = withCheck(form);
    expect(form.checks).toHaveLength(8);
    expect(withCheck(form)).toBe(form);
    expect(withoutCheck(form, 1).checks).toHaveLength(7);
  });

  it('makes unique IDs from names', () => {
    expect(newScenarioId('Asks to be called back!', [])).toBe('asks-to-be-called-back');
    expect(newScenarioId('Books', ['books', 'books-2'])).toBe('books-3');
    expect(newScenarioId('2nd try', [])).toBe('scenario-2nd-try');
    expect(newScenarioId('a'.repeat(50), []).length).toBeLessThanOrEqual(40);
  });

  it('replaces, adds and removes scenarios, and keeps the set valid', () => {
    const other = { ...scenario, id: 'other', required: false };
    expect(withScenario([scenario], other)).toEqual([scenario, other]);
    expect(withScenario([scenario, other], { ...other, name: 'Renamed' })[1]?.name).toBe('Renamed');
    expect(withoutScenario([scenario, other], 'books')).toEqual([other]);
    expect(setProblem([scenario, other])).toBeNull();
    expect(setProblem([])).toBe('Keep at least one scenario.');
    expect(setProblem([other])).toMatch(/^Keep at least one scenario required/);
    expect(setProblem(Array.from({ length: 9 }, (_, i) => ({ ...scenario, id: `s${i}` })))).toBe(
      'Keep at most 8 scenarios.',
    );
  });
});

const line = (turn: number, replyMs: number | null): TranscriptLine => ({
  key: `agent-${turn}`,
  turn,
  speaker: 'agent',
  text: 'Hi.',
  interrupted: false,
  replyMs,
});
const ended = (lines: TranscriptLine[]): TestCallState => ({
  ...initialCall,
  phase: 'ended',
  lines,
});

describe('a call summary', () => {
  it('takes nearest-rank percentiles', () => {
    expect(percentile([], 0.5)).toBeNull();
    expect(percentile([820, 1_240], 0.5)).toBe(820);
    expect(percentile([820, 1_240], 0.95)).toBe(1_240);
    expect(percentile([100, 200, 300, 400], 0.5)).toBe(200);
    expect(percentile([5], 0)).toBe(5);
  });

  it('sums up an ended call that had lines', () => {
    expect(callSummaryOf(ended([line(0, null), line(2, 1_240), line(4, 820)]))).toEqual({
      turns: 3,
      p50Ms: 820,
      p95Ms: 1_240,
    });
    expect(callSummaryOf(ended([line(0, null)]))).toEqual({ turns: 1, p50Ms: null, p95Ms: null });
    expect(callSummaryOf(ended([]))).toBeNull();
    expect(callSummaryOf({ ...ended([line(0, 10)]), phase: 'live' })).toBeNull();
  });
});
