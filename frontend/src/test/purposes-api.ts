// An in-memory fake of the /api/v1 purposes endpoints and the tool catalog
// (MSW), shaped like contracts/openapi.json. Tests outside features/*/api use it instead of the
// generated mocks; a test overrides one endpoint with `server.use(...)` to fail it.
import { HttpResponse, http } from 'msw';

import { SYSTEM_PURPOSE_IDS, problemResponse } from './agents-api';
import { LOCKED_TEXTS } from './plans-api';

export const CUSTOM_ID = '019a5a5a-0000-7000-8000-000000000001';

const NAMES: Record<keyof typeof SYSTEM_PURPOSE_IDS, string> = {
  support: 'Customer support',
  service: 'Customer service',
  booking: 'Appointment booking',
  survey: 'Survey',
  telemarketing: 'Outbound sales',
  custom: 'Something else',
};

/** A purpose's draft as the API sends it, by field; `overrides` replace whole fields. */
function draftBody(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    name: 'Clinic bookings',
    description: '',
    complianceClass: 'booking',
    persona: 'Your job: book, move and cancel appointments.',
    tools: { builtins: ['mark_do_not_call'], http: [] },
    guardrails: { allowedTopics: [], never: [], escalations: [], maxCallMinutes: 30, neverAsk: [] },
    compliance: {
      optOutPhrases: [],
      forbiddenClaims: [],
      recordingsDays: null,
      transcriptsDays: null,
    },
    ...overrides,
  };
}

const RETENTION = { recordingsDays: 180, transcriptsDays: 365, auditDays: 1826 };

function baselineBody(jurisdiction: string) {
  return {
    jurisdiction,
    rulePack: { id: `${jurisdiction.toLowerCase()}-booking`, version: 1 },
    lockedTexts: LOCKED_TEXTS,
    optOutPhrases: ['stop calling me'],
    optOutConfirmation: "Understood. We won't contact you again. Goodbye.",
    forbiddenClaims: ['Saying it is a human.'],
    retention: RETENTION,
    effective: {
      optOutPhrases: ['stop calling me'],
      forbiddenClaims: ['Saying it is a human.'],
      retention: RETENTION,
    },
  };
}

/** A purpose as `GET /purposes/:id` answers it. */
export interface PurposeBody {
  id: string;
  system: boolean;
  latestVersion: number | null;
  unsavedChanges: boolean;
  usedBy: number;
  draft: Record<string, unknown>;
  problems: { code: string; message: string; field: string }[];
  baselines: ReturnType<typeof baselineBody>[];
}

export function purposeBody(overrides: Partial<PurposeBody> = {}): PurposeBody {
  return {
    id: CUSTOM_ID,
    system: false,
    latestVersion: null,
    unsavedChanges: true,
    usedBy: 0,
    draft: draftBody(),
    problems: [],
    baselines: [baselineBody('US'), baselineBody('US-CA')],
    ...overrides,
  };
}

export function systemPurposeBody(useCase: keyof typeof SYSTEM_PURPOSE_IDS) {
  return purposeBody({
    id: SYSTEM_PURPOSE_IDS[useCase],
    system: true,
    latestVersion: 1,
    unsavedChanges: false,
    draft: draftBody({ name: NAMES[useCase], complianceClass: useCase }),
  });
}

export function toolCatalogBody() {
  return {
    builtins: [
      {
        name: 'transfer_to_human' as const,
        label: 'Transfer to a person',
        description: 'Hand over.',
        locked: false,
      },
      { name: 'end_call' as const, label: 'End the call', description: 'End it.', locked: false },
      {
        name: 'mark_do_not_call' as const,
        label: 'Opt out',
        description: 'Record an opt-out.',
        locked: true,
      },
    ],
    allowedHosts: ['calendar.example.com', '*.tools.test'],
  };
}

function summaryOf(purpose: PurposeBody) {
  return {
    id: purpose.id,
    name: purpose.draft['name'],
    description: purpose.draft['description'],
    complianceClass: purpose.draft['complianceClass'],
    system: purpose.system,
    latestVersion: purpose.latestVersion,
    unsavedChanges: purpose.unsavedChanges,
    usedBy: purpose.usedBy,
  };
}

export async function objectBody(request: Request): Promise<Record<string, unknown>> {
  const body: unknown = await request.json();
  return typeof body === 'object' && body !== null ? Object.fromEntries(Object.entries(body)) : {};
}

/** Handlers over a store of purposes; `patches` records every PATCH body. */
export function fakePurposesApi(initial: PurposeBody[] = []) {
  const purposes = new Map<string, PurposeBody>(
    [systemPurposeBody('support'), systemPurposeBody('booking'), ...initial].map((purpose) => [
      purpose.id,
      purpose,
    ]),
  );
  const patches: Record<string, unknown>[] = [];
  const find = (id: unknown) => (typeof id === 'string' ? purposes.get(id) : undefined);
  const handlers = [
    http.get('*/api/v1/purposes', () =>
      HttpResponse.json({ items: [...purposes.values()].map(summaryOf) }),
    ),
    http.post('*/api/v1/purposes', async ({ request }) => {
      const body = await objectBody(request);
      const source = find(body['duplicateOf']);
      if (source === undefined)
        return problemResponse(422, [
          { field: 'duplicateOf', message: 'Choose a purpose to copy.' },
        ]);
      const name = body['name'] ?? `Copy of ${String(source.draft['name'])}`;
      const created = purposeBody({ id: CUSTOM_ID, draft: { ...source.draft, name } });
      purposes.set(created.id, created);
      return HttpResponse.json(created, { status: 201 });
    }),
    http.get('*/api/v1/purposes/:id', ({ params }) => {
      const purpose = find(params['id']);
      return purpose === undefined ? problemResponse(404) : HttpResponse.json(purpose);
    }),
    http.patch('*/api/v1/purposes/:id', async ({ params, request }) => {
      const purpose = find(params['id']);
      if (purpose === undefined) return problemResponse(404);
      if (purpose.system) return problemResponse(409);
      const body = await objectBody(request);
      patches.push(body);
      const updated = { ...purpose, draft: { ...purpose.draft, ...body }, unsavedChanges: true };
      purposes.set(purpose.id, updated);
      return HttpResponse.json(updated);
    }),
    http.post('*/api/v1/purposes/:id/versions', ({ params }) => {
      const purpose = find(params['id']);
      if (purpose === undefined) return problemResponse(404);
      const saved = {
        ...purpose,
        latestVersion: (purpose.latestVersion ?? 0) + 1,
        unsavedChanges: false,
      };
      purposes.set(purpose.id, saved);
      return HttpResponse.json(saved, { status: 201 });
    }),
    http.get('*/api/v1/catalog/tools', () => HttpResponse.json(toolCatalogBody())),
  ];
  return { handlers, purposes, patches };
}
