// An in-memory fake of the /api/v1 agents and catalog endpoints (MSW), shaped like
// contracts/openapi.json. Tests outside features/*/api may not import the generated client or
// its mocks, so they use this; a test overrides one endpoint with `server.use(...)` to fail it.
import { HttpResponse, http } from 'msw';

export const AGENT_ID = '01920000-0000-7000-8000-000000000001';

export function agentBody(overrides: Record<string, unknown> = {}) {
  return {
    id: AGENT_ID,
    name: 'Front desk',
    useCase: 'support' as const,
    voice: 'af_heart',
    language: 'en-US',
    jurisdiction: 'US',
    rulePack: { id: 'us-support', version: 1 },
    model: 'balanced' as const,
    turnDetection: { silenceMs: 500, bargeIn: true },
    draftHash: `sha256:${'a'.repeat(64)}`,
    createdAt: '2026-10-30T10:00:00Z',
    updatedAt: '2026-10-30T10:00:00Z',
    archivedAt: null as string | null,
    ...overrides,
  };
}

export function progressBody() {
  return {
    percent: 25,
    steps: [
      { step: 'provision' as const, done: true },
      { step: 'plan' as const, done: false },
      { step: 'test' as const, done: false },
      { step: 'publish' as const, done: false },
    ],
    nextStep: 'plan' as const,
  };
}

export function voicesBody() {
  return {
    items: [
      { id: 'af_heart', name: 'Heart', description: 'American English, female', language: 'en-US' },
      { id: 'bm_george', name: 'George', description: 'British English, male', language: 'en-GB' },
    ],
  };
}

export function languagesBody() {
  return {
    items: [
      { code: 'en-US', name: 'English (United States)' },
      { code: 'en-GB', name: 'English (United Kingdom)' },
    ],
  };
}

/** An RFC 9457 problem response. */
export function problemResponse(status: number, errors: { field: string; message: string }[] = []) {
  return HttpResponse.json(
    { type: 'about:blank', title: 'Error', status, detail: 'The request failed.', errors },
    { status, headers: { 'content-type': 'application/problem+json' } },
  );
}

type AgentBody = ReturnType<typeof agentBody>;

async function readObject(request: Request): Promise<Record<string, unknown>> {
  const body: unknown = await request.json();
  if (typeof body !== 'object' || body === null || Array.isArray(body)) {
    throw new Error('expected a JSON object body');
  }
  return Object.fromEntries(Object.entries(body));
}

/** Handlers over a mutable store of agents; `requests` records every PATCH body. */
export function fakeAgentsApi(initial: AgentBody[] = []) {
  const agents = new Map(initial.map((agent) => [agent.id, agent]));
  const patches: unknown[] = [];
  const find = (id: unknown) => (typeof id === 'string' ? agents.get(id) : undefined);
  const handlers = [
    http.get('*/api/v1/agents', () =>
      HttpResponse.json({
        items: [...agents.values()]
          .filter((agent) => agent.archivedAt === null)
          .map(({ id, name, useCase, voice, updatedAt }) => ({
            id,
            name,
            useCase,
            voice,
            updatedAt,
            progressPercent: 25,
          })),
      }),
    ),
    http.post('*/api/v1/agents', async ({ request }) => {
      const body = await readObject(request);
      const created = agentBody({ ...body });
      agents.set(created.id, created);
      return HttpResponse.json(created, { status: 201 });
    }),
    http.get('*/api/v1/agents/:id', ({ params }) => {
      const agent = find(params['id']);
      return agent === undefined ? problemResponse(404) : HttpResponse.json(agent);
    }),
    http.patch('*/api/v1/agents/:id', async ({ params, request }) => {
      const agent = find(params['id']);
      if (agent === undefined) return problemResponse(404);
      const changes = await readObject(request);
      patches.push(changes);
      const updated = { ...agent, ...changes };
      agents.set(agent.id, updated);
      return HttpResponse.json(updated);
    }),
    http.delete('*/api/v1/agents/:id', ({ params }) => {
      const agent = find(params['id']);
      if (agent === undefined) return problemResponse(404);
      agents.set(agent.id, { ...agent, archivedAt: '2026-10-30T11:00:00Z' });
      return new HttpResponse(null, { status: 204 });
    }),
    http.get('*/api/v1/agents/:id/progress', ({ params }) =>
      find(params['id']) === undefined ? problemResponse(404) : HttpResponse.json(progressBody()),
    ),
    http.get('*/api/v1/catalog/voices', () => HttpResponse.json(voicesBody())),
    http.get('*/api/v1/catalog/languages', () => HttpResponse.json(languagesBody())),
  ];
  return { handlers, agents, patches };
}
