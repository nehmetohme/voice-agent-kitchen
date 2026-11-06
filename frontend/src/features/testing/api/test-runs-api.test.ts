import { HttpResponse, http } from 'msw';
import { describe, expect, it } from 'vitest';

import {
  getCancelTestRunMockHandler,
  getGetAgentScenariosMockHandler,
  getGetTestRunMockHandler,
  getListAgentTestRunsMockHandler,
  getSaveAgentScenariosMockHandler,
  getStartTestRunMockHandler,
} from '../../../shared/api/generated/endpoints/testing/testing.msw';
import { ApiError } from '../../../shared/api/http';
import { AGENT_ID } from '../../../test/agents-api';
import { startMswServer } from '../../../test/msw-server';
import {
  cancelTestRun,
  getScenarios,
  getTestRun,
  isNotFound,
  isSendable,
  listTestRuns,
  saveScenarios,
  startProblemOf,
  startTestRun,
  type Scenario,
} from './test-runs-api';

const server = startMswServer();
const RUN_ID = '01920000-0000-7000-8000-0000000000bb';

const scenario: Scenario = {
  id: 'books',
  name: 'Books',
  goal: 'Book a table.',
  persona: '',
  required: true,
  maxTurns: 6,
  rubric: [{ question: 'Booked?', expected: 'yes' }],
};

const runBody = {
  id: RUN_ID,
  agentId: AGENT_ID,
  status: 'finished' as const,
  verdict: 'passed' as const,
  draftHash: `sha256:${'a'.repeat(64)}`,
  progress: { done: 1, total: 1, percent: 100 },
  models: { llm: 'KAT-Coder', decisions: 'Kev-4B', seed: 1 },
  createdAt: '2026-11-06T10:00:00Z',
  startedAt: '2026-11-06T10:00:01Z',
  finishedAt: '2026-11-06T10:00:09Z',
  scenarios: [
    {
      ...scenario,
      version: `sha256:${'c'.repeat(64)}`,
      status: 'passed' as const,
      endReason: 'opt_out' as const,
      optOutLayer: 'phrase' as const,
      durationMs: 1_200,
      transcript: [{ speaker: 'agent' as const, text: 'Hi.' }],
      checks: [{ question: 'Booked?', expected: 'yes' as const, probability: 0.9, passed: true }],
    },
  ],
};

function problem(status: number, detail?: string) {
  return new ApiError(status, { type: 'about:blank', title: 'Problem', status, detail });
}

describe('test runs API', () => {
  it('reads and saves the scenarios', async () => {
    server.use(getGetAgentScenariosMockHandler({ saved: false, scenarios: [scenario] }));
    await expect(getScenarios(AGENT_ID)).resolves.toEqual({ saved: false, scenarios: [scenario] });

    let sent: unknown = null;
    server.use(
      getSaveAgentScenariosMockHandler(async ({ request }) => {
        sent = await request.json();
        return { saved: true, scenarios: [scenario] };
      }),
    );
    await expect(saveScenarios(AGENT_ID, [scenario])).resolves.toEqual({
      saved: true,
      scenarios: [scenario],
    });
    expect(sent).toEqual({ scenarios: [scenario] });
    expect(isSendable([scenario])).toBe(true);
    expect(isSendable([])).toBe(false);
    expect(isSendable([{ ...scenario, name: '' }])).toBe(false);
  });

  it('starts, reads and cancels runs, and lists them', async () => {
    server.use(
      getStartTestRunMockHandler({ ...runBody, status: 'queued', verdict: null, models: null }),
      getGetTestRunMockHandler(runBody),
      getCancelTestRunMockHandler({ ...runBody, status: 'cancelled', verdict: null }),
      getListAgentTestRunsMockHandler({
        draftHash: runBody.draftHash,
        items: [
          {
            id: RUN_ID,
            status: 'finished',
            verdict: 'passed',
            draftHash: runBody.draftHash,
            current: false,
            progress: runBody.progress,
            createdAt: runBody.createdAt,
            finishedAt: runBody.finishedAt,
          },
        ],
      }),
    );
    await expect(startTestRun(AGENT_ID)).resolves.toMatchObject({
      status: 'queued',
      verdict: null,
      models: null,
    });
    const read = await getTestRun(RUN_ID);
    expect(read).toEqual({
      id: RUN_ID,
      status: 'finished',
      verdict: 'passed',
      draftHash: runBody.draftHash,
      progress: runBody.progress,
      models: runBody.models,
      scenarios: [
        {
          id: 'books',
          name: 'Books',
          required: true,
          status: 'passed',
          endReason: 'opt_out',
          optOutLayer: 'phrase',
          durationMs: 1_200,
          transcript: [{ speaker: 'agent', text: 'Hi.' }],
          checks: [{ question: 'Booked?', expected: 'yes', probability: 0.9, passed: true }],
        },
      ],
    });
    await expect(cancelTestRun(RUN_ID)).resolves.toMatchObject({ status: 'cancelled' });
    await expect(listTestRuns(AGENT_ID)).resolves.toEqual([
      { id: RUN_ID, status: 'finished', current: false },
    ]);
  });

  it('reads why a run could not start', async () => {
    expect(startProblemOf(problem(409, 'A test run of this agent is already queued.'))).toBe(
      'A test run of this agent is already queued.',
    );
    expect(startProblemOf(problem(500, 'internal'))).toMatch(/^Couldn't start the test run/);
    expect(startProblemOf(new Error('offline'))).toMatch(/^Couldn't start the test run/);
    expect(isNotFound(problem(404))).toBe(true);
    expect(isNotFound(problem(503))).toBe(false);

    server.use(
      http.post('*/api/v1/test-runs', () =>
        HttpResponse.json(
          { type: 'about:blank', title: 'Conflict', status: 409, detail: 'Busy.' },
          { status: 409, headers: { 'content-type': 'application/problem+json' } },
        ),
      ),
    );
    await expect(startTestRun(AGENT_ID)).rejects.toMatchObject({ status: 409 });
  });
});
