// An in-memory fake of the scenarios and test-run endpoints (MSW), shaped like
// contracts/openapi.json, for tests outside features/*/api. A run is queued by POST and is
// finished the first time it is read again, unless `holdRuns` keeps it running.
import { HttpResponse, http } from 'msw';

import { AGENT_ID } from './agents-api';

const RUN_ID = '01920000-0000-7000-8000-0000000000bb';
const DRAFT_HASH = `sha256:${'b'.repeat(64)}`;

export interface FakeScenario {
  id: string;
  name: string;
  goal: string;
  persona: string;
  required: boolean;
  maxTurns: number;
  rubric: { question: string; expected: 'yes' | 'no' }[];
}

function scenarioBody(id: string, name: string, required: boolean): FakeScenario {
  return {
    id,
    name,
    goal: `The goal of ${name}.`,
    persona: 'Polite.',
    required,
    maxTurns: 6,
    rubric: [{ question: `Did ${name} work?`, expected: 'yes' }],
  };
}

const DEFAULTS = [
  scenarioBody('completes-survey', 'Completes the survey', true),
  scenarioBody('asks-to-call-back', 'Asks to be called back', false),
];

function isScenarioList(value: unknown): value is FakeScenario[] {
  return (
    Array.isArray(value) &&
    value.every((item: unknown) => typeof item === 'object' && item !== null && 'id' in item)
  );
}

/** A result of `scenario`: passed, or failed when `passed` is false. */
function resultBody(scenario: FakeScenario, status: string) {
  const played = status === 'passed' || status === 'failed';
  return {
    ...scenario,
    version: `sha256:${'c'.repeat(64)}`,
    status,
    endReason: played ? 'caller_hung_up' : null,
    optOutLayer: null,
    durationMs: played ? 3_200 : null,
    transcript: played
      ? [
          { speaker: 'agent', text: 'Hi, this is Front desk, an automated AI assistant.' },
          { speaker: 'caller', text: `Hello from ${scenario.name}.` },
        ]
      : [],
    checks: played
      ? scenario.rubric.map((item) => ({
          ...item,
          probability: status === 'passed' ? 0.9 : 0.1,
          passed: status === 'passed',
        }))
      : [],
  };
}

interface FakeRun {
  status: 'queued' | 'running' | 'finished' | 'cancelled' | 'interrupted';
  scenarios: FakeScenario[];
  failing: string[];
}

function runBody(run: FakeRun) {
  const finished = run.status === 'finished';
  const statusOf = (scenario: FakeScenario) => {
    if (finished) return run.failing.includes(scenario.id) ? 'failed' : 'passed';
    if (run.status === 'running') return 'running';
    return run.status === 'queued' ? 'pending' : 'skipped';
  };
  const results = run.scenarios.map((scenario) => resultBody(scenario, statusOf(scenario)));
  const requiredPassed = results.every((result) => !result.required || result.status === 'passed');
  const done = results.filter((result) => !['pending', 'running'].includes(result.status)).length;
  return {
    id: RUN_ID,
    agentId: AGENT_ID,
    status: run.status,
    verdict: finished ? (requiredPassed ? 'passed' : 'failed') : null,
    draftHash: DRAFT_HASH,
    progress: {
      done,
      total: results.length,
      percent: Math.floor((done * 100) / Math.max(1, results.length)),
    },
    models:
      run.status === 'queued' ? null : { llm: 'fake-llm', decisions: 'fake-decisions', seed: 1 },
    createdAt: '2026-11-06T10:00:00Z',
    startedAt: run.status === 'queued' ? null : '2026-11-06T10:00:01Z',
    finishedAt: finished ? '2026-11-06T10:00:09Z' : null,
    scenarios: results,
  };
}

/** Handlers and the fake's state: what was saved and started, and switches for tests. */
export function fakeTestRunsApi() {
  const state = {
    scenarios: [...DEFAULTS],
    saved: false,
    run: null as FakeRun | null,
    /** Keep runs running when read, instead of finishing them. */
    holdRuns: false,
    /** Scenario IDs that fail when a run finishes. */
    failing: [] as string[],
    /** Whether the latest run tested the current draft. */
    current: true,
    savedBodies: [] as unknown[],
    started: 0,
  };
  const handlers = [
    http.get('*/api/v1/agents/:id/scenarios', () =>
      HttpResponse.json({ saved: state.saved, scenarios: state.scenarios }),
    ),
    http.put('*/api/v1/agents/:id/scenarios', async ({ request }) => {
      const body: unknown = await request.json();
      state.savedBodies.push(body);
      const scenarios: unknown =
        typeof body === 'object' && body !== null && 'scenarios' in body ? body.scenarios : [];
      state.scenarios = isScenarioList(scenarios) ? scenarios : [];
      state.saved = true;
      return HttpResponse.json({ saved: true, scenarios: state.scenarios });
    }),
    http.get('*/api/v1/agents/:id/test-runs', () =>
      HttpResponse.json({
        draftHash: DRAFT_HASH,
        items:
          state.run === null
            ? []
            : [
                {
                  id: RUN_ID,
                  status: state.run.status,
                  verdict: runBody(state.run).verdict,
                  draftHash: DRAFT_HASH,
                  current: state.current,
                  progress: runBody(state.run).progress,
                  createdAt: '2026-11-06T10:00:00Z',
                  finishedAt: null,
                },
              ],
      }),
    ),
    http.post('*/api/v1/test-runs', () => {
      state.started += 1;
      state.run = { status: 'queued', scenarios: state.scenarios, failing: state.failing };
      return HttpResponse.json(runBody(state.run), { status: 202 });
    }),
    http.get('*/api/v1/test-runs/:id', () => {
      if (state.run === null) return HttpResponse.json({}, { status: 404 });
      if (state.run.status === 'queued' || state.run.status === 'running') {
        state.run.status = state.holdRuns ? 'running' : 'finished';
      }
      return HttpResponse.json(runBody(state.run));
    }),
    http.post('*/api/v1/test-runs/:id/cancel', () => {
      if (state.run === null) return HttpResponse.json({}, { status: 404 });
      state.run.status = 'cancelled';
      return HttpResponse.json(runBody(state.run));
    }),
  ];
  return { handlers, state };
}
