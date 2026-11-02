// An in-memory fake of `GET/PUT /api/v1/agents/:id/plan` (MSW), shaped like
// contracts/openapi.json. Tests outside features/*/api may not import the generated client or
// its mocks, so they use this. Saved plans are checked by `validate`, which a test can swap.
import { HttpResponse, http } from 'msw';

import { problemResponse } from './agents-api';

export const LOCKED_TEXTS = [
  { key: 'ai_disclosure', text: "Hello, you're speaking with an AI assistant." },
  { key: 'recording_notice', text: 'This call is recorded and transcribed.' },
];

export interface FakeStep {
  id: string;
  kind: 'start' | 'say' | 'ask' | 'collect' | 'tool' | 'transfer' | 'end';
  label: string;
  text: string;
}

export interface FakePath {
  id: string;
  from: string;
  to: string;
  condition: string;
}

export interface FakeProblem {
  code: string;
  message: string;
  nodeId?: string;
  edgeId?: string;
}

/** A three-step template: greeting → question → goodbye. */
export function templateBody() {
  return {
    nodes: [
      {
        id: 'start',
        kind: 'start',
        label: 'Greeting',
        text: `${LOCKED_TEXTS.map(({ text }) => text).join(' ')} How can I help?`,
      },
      { id: 'ask', kind: 'ask', label: 'Ask what they need', text: 'Ask what they need.' },
      { id: 'goodbye', kind: 'end', label: 'Goodbye', text: 'Say goodbye.' },
    ] satisfies FakeStep[],
    edges: [
      { id: 'p1', from: 'start', to: 'ask', condition: 'The caller answers' },
      { id: 'p2', from: 'ask', to: 'goodbye', condition: 'The caller is done' },
    ] satisfies FakePath[],
  };
}

/** The fake's rule: every step but an end needs a path out; nothing else. */
function exitsOnly(plan: { nodes: FakeStep[]; edges: FakePath[] }): FakeProblem[] {
  return plan.nodes
    .filter((node) => node.kind !== 'end' && !plan.edges.some((edge) => edge.from === node.id))
    .map((node) => ({
      code: 'missing_exit',
      message: 'This step has no path out. Add one, or make it an end step.',
      nodeId: node.id,
    }));
}

function planBody(
  plan: { nodes: FakeStep[]; edges: FakePath[] },
  saved: boolean,
  errors: FakeProblem[],
) {
  return {
    saved,
    rulePack: { id: 'us-support', version: 1 },
    lockedTexts: LOCKED_TEXTS,
    optOutConfirmation: "Understood. We won't contact you again. Goodbye.",
    nodes: plan.nodes,
    edges: plan.edges,
    validation: { valid: errors.length === 0, errors },
  };
}

function isPlan(body: unknown): body is { nodes: FakeStep[]; edges: FakePath[] } {
  return (
    typeof body === 'object' &&
    body !== null &&
    'nodes' in body &&
    Array.isArray(body.nodes) &&
    'edges' in body &&
    Array.isArray(body.edges)
  );
}

/** Handlers over one agent's plan; `saves` records every PUT body. */
export function fakePlansApi(agentId: string, validate = exitsOnly) {
  let stored: { nodes: FakeStep[]; edges: FakePath[] } | null = null;
  const saves: unknown[] = [];
  const handlers = [
    http.get('*/api/v1/agents/:id/plan', ({ params }) => {
      if (params['id'] !== agentId) return problemResponse(404);
      const plan = stored ?? templateBody();
      return HttpResponse.json(planBody(plan, stored !== null, validate(plan)));
    }),
    http.put('*/api/v1/agents/:id/plan', async ({ params, request }) => {
      if (params['id'] !== agentId) return problemResponse(404);
      const body: unknown = await request.json();
      if (!isPlan(body)) return problemResponse(422);
      saves.push(body);
      stored = body;
      return HttpResponse.json(planBody(body, true, validate(body)));
    }),
  ];
  return { handlers, saves };
}
