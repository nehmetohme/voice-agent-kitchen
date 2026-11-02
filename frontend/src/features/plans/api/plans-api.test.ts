import { http } from 'msw';
import { describe, expect, it } from 'vitest';

import {
  getGetAgentPlanMockHandler,
  getSaveAgentPlanMockHandler,
} from '../../../shared/api/generated/endpoints/plans/plans.msw';
import { problemResponse } from '../../../test/agents-api';
import { startMswServer } from '../../../test/msw-server';
import { LOCKED_TEXTS, templateBody } from '../../../test/plans-api';
import { getPlan, isNotFound, isSendable, PLAN_LIMITS, savePlan } from './plans-api';

const server = startMswServer();
const AGENT = '01920000-0000-7000-8000-000000000001';

const answer = {
  saved: true,
  rulePack: { id: 'us-support', version: 1 },
  lockedTexts: LOCKED_TEXTS,
  optOutConfirmation: null,
  ...templateBody(),
  validation: {
    valid: false,
    errors: [
      { code: 'missing_start', message: 'Add a start step.' },
      { code: 'empty_condition', message: 'Say when.', edgeId: 'p1' },
      { code: 'missing_exit', message: 'Add a path out.', nodeId: 'ask', edgeId: null },
    ],
  },
};

describe('plans api', () => {
  it('maps the plan to steps, paths and located problems', async () => {
    server.use(getGetAgentPlanMockHandler(answer));

    const plan = await getPlan(AGENT);

    expect(plan.saved).toBe(true);
    expect(plan.steps.map((step) => step.id)).toEqual(['start', 'ask', 'goodbye']);
    expect(plan.paths[0]).toEqual({
      id: 'p1',
      from: 'start',
      to: 'ask',
      condition: 'The caller answers',
    });
    expect(plan.lockedTexts).toEqual(LOCKED_TEXTS);
    expect(plan.optOutConfirmation).toBeNull();
    expect(plan.valid).toBe(false);
    expect(plan.problems).toEqual([
      { code: 'missing_start', message: 'Add a start step.', stepId: null, pathId: null },
      { code: 'empty_condition', message: 'Say when.', stepId: null, pathId: 'p1' },
      { code: 'missing_exit', message: 'Add a path out.', stepId: 'ask', pathId: null },
    ]);
  });

  it('sends the whole plan, and refuses a body the contract rejects before it leaves', async () => {
    let sent: unknown;
    server.use(
      getSaveAgentPlanMockHandler(async ({ request }) => {
        sent = await request.json();
        return answer;
      }),
    );
    const { nodes, edges } = templateBody();
    const draft = { steps: nodes, paths: edges };

    await savePlan(AGENT, draft);

    expect(sent).toEqual({ nodes, edges });
    const nameless = {
      ...draft,
      steps: [{ ...nodes[0], id: 'start', kind: 'start' as const, label: '', text: '' }],
    };
    expect(isSendable(draft)).toBe(true);
    expect(isSendable(nameless)).toBe(false);
    await expect(savePlan(AGENT, nameless)).rejects.toThrow(/too_small/);
  });

  it('knows a missing agent and the contract limits', async () => {
    server.use(http.get('*/api/v1/agents/:id/plan', () => problemResponse(404)));

    const error: unknown = await getPlan(AGENT).catch((caught: unknown) => caught);

    expect(isNotFound(error)).toBe(true);
    expect(isNotFound(new Error('offline'))).toBe(false);
    expect(PLAN_LIMITS).toEqual({ steps: 50, label: 80, text: 2_000, condition: 200 });
  });
});
