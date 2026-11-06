import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';

import { getListToolsMockHandler } from '../../../shared/api/generated/endpoints/catalog/catalog.msw';
import {
  getCreatePurposeMockHandler,
  getGetPurposeMockHandler,
  getListPurposesMockHandler,
  getSavePurposeVersionMockHandler,
} from '../../../shared/api/generated/endpoints/purposes/purposes.msw';
import type { HttpTool, Purpose } from '../../../shared/api/generated/model';
import { ApiError } from '../../../shared/api/http';
import { problemResponse } from '../../../test/agents-api';
import { startMswServer } from '../../../test/msw-server';
import { LOCKED_TEXTS } from '../../../test/plans-api';
import { CUSTOM_ID, toolCatalogBody } from '../../../test/purposes-api';
import { isSendable } from './purpose-mapping';
import type { HttpToolDraft } from './purpose-models';
import {
  duplicatePurpose,
  fieldErrorsOf,
  getPurpose,
  isNotFound,
  listPurposes,
  listTools,
  problemDetail,
  savePurposeVersion,
  updatePurpose,
} from './purposes-api';

const server = startMswServer();

const tool: HttpTool = {
  name: 'book_slot',
  description: 'Books a slot.',
  method: 'POST',
  url: 'https://calendar.example.com/v1/slots',
  timeoutMs: 3000,
  secret: 'CALENDAR_API_KEY',
  confirm: true,
  inputs: {
    type: 'object',
    properties: {
      date: { type: 'string', description: 'The day.' },
      people: { type: 'integer', description: 'How many.' },
    },
    required: ['date'],
  },
};

const RETENTION = { recordingsDays: 180, transcriptsDays: 365, auditDays: 1826 };

const stored: Purpose = {
  id: CUSTOM_ID,
  system: false,
  latestVersion: 2,
  unsavedChanges: true,
  usedBy: 0,
  draft: {
    name: 'Clinic bookings',
    description: '',
    complianceClass: 'booking',
    persona: 'Book.',
    tools: { builtins: ['mark_do_not_call', 'end_call'], http: [tool] },
    guardrails: {
      allowedTopics: ['Appointments'],
      never: ['Give medical advice.'],
      escalations: [
        { when: 'Is the caller upset?', action: 'transfer', message: null },
        { when: 'Is it an emergency?', action: 'end_call', message: 'Please call 911.' },
      ],
      maxCallMinutes: 10,
      neverAsk: ['health'],
    },
    compliance: {
      optOutPhrases: [],
      forbiddenClaims: [],
      recordingsDays: null,
      transcriptsDays: 30,
    },
  },
  problems: [
    {
      code: 'host_not_allowed',
      message: 'Not allowed.',
      field: 'tools.http[0].url',
      nodeId: null,
      edgeId: null,
    },
  ],
  baselines: [
    {
      jurisdiction: 'US',
      rulePack: { id: 'us-booking', version: 1 },
      lockedTexts: LOCKED_TEXTS,
      optOutPhrases: ['stop calling me'],
      optOutConfirmation: "Understood. We won't contact you again.",
      forbiddenClaims: ['Saying it is a human.'],
      retention: RETENTION,
      effective: {
        optOutPhrases: ['stop calling me'],
        forbiddenClaims: ['Saying it is a human.'],
        retention: { ...RETENTION, transcriptsDays: 30 },
      },
    },
  ],
};

describe('purposes api', () => {
  it('lists purposes as the list shows them', async () => {
    server.use(
      getListPurposesMockHandler({
        items: [
          {
            id: CUSTOM_ID,
            name: 'Clinic bookings',
            description: 'Dental.',
            complianceClass: 'booking',
            system: false,
            latestVersion: null,
            unsavedChanges: true,
            usedBy: 3,
          },
        ],
      }),
    );

    expect(await listPurposes()).toEqual([
      {
        id: CUSTOM_ID,
        name: 'Clinic bookings',
        description: 'Dental.',
        complianceClass: 'booking',
        system: false,
        latestVersion: null,
        unsavedChanges: true,
        usedBy: 3,
      },
    ]);
  });

  it('maps a purpose to the editor sections, its problems and baselines', async () => {
    server.use(getGetPurposeMockHandler(stored));

    const purpose = await getPurpose(CUSTOM_ID);

    expect(purpose.settings.tools.http[0]).toEqual({
      name: 'book_slot',
      description: 'Books a slot.',
      method: 'POST',
      url: 'https://calendar.example.com/v1/slots',
      timeoutMs: 3000,
      secret: 'CALENDAR_API_KEY',
      confirm: true,
      inputs: [
        { name: 'date', type: 'string', description: 'The day.', required: true },
        { name: 'people', type: 'integer', description: 'How many.', required: false },
      ],
    });
    expect(purpose.settings.guardrails.escalations).toEqual([
      { when: 'Is the caller upset?', action: 'transfer', message: '' },
      { when: 'Is it an emergency?', action: 'end_call', message: 'Please call 911.' },
    ]);
    expect(purpose.problems).toEqual([
      { code: 'host_not_allowed', message: 'Not allowed.', field: 'tools.http[0].url' },
    ]);
    expect(purpose.baselines[0]?.rulePack).toBe('us-booking v1');
    expect(purpose.baselines[0]?.lockedTexts[0]).toBe(
      "Hello, you're speaking with an AI assistant.",
    );
  });

  it('sends the changed sections in the contract shape', async () => {
    server.use(getGetPurposeMockHandler(stored));
    const { settings } = await getPurpose(CUSTOM_ID);
    let sent: unknown;
    server.use(
      http.patch('*/api/v1/purposes/:id', async ({ request }) => {
        sent = await request.json();
        return HttpResponse.json(stored);
      }),
    );
    const [first] = settings.tools.http;
    if (first === undefined) throw new Error('The stored purpose has a tool.');
    const noSecret: HttpToolDraft = { ...first, name: 'find', secret: ' ' };

    await updatePurpose(CUSTOM_ID, {
      tools: { ...settings.tools, http: [first, noSecret] },
      guardrails: settings.guardrails,
    });

    expect(sent).toEqual({
      tools: {
        builtins: ['mark_do_not_call', 'end_call'],
        http: [tool, { ...tool, name: 'find', secret: undefined }],
      },
      guardrails: {
        ...stored.draft.guardrails,
        escalations: [
          { when: 'Is the caller upset?', action: 'transfer' },
          { when: 'Is it an emergency?', action: 'end_call', message: 'Please call 911.' },
        ],
      },
    });
  });

  it('refuses to send a section outside the contract', async () => {
    server.use(getGetPurposeMockHandler(stored));
    const { settings } = await getPurpose(CUSTOM_ID);
    const nameless = { overview: { ...settings.overview, name: '' } };

    expect(isSendable({ overview: settings.overview })).toBe(true);
    expect(isSendable(nameless)).toBe(false);
    await expect(updatePurpose(CUSTOM_ID, nameless)).rejects.toThrow(/too_small/);
  });

  it('copies a purpose and saves a version', async () => {
    let sent: unknown;
    server.use(
      http.post('*/api/v1/purposes', async ({ request }) => {
        sent = await request.json();
        return HttpResponse.json(stored, { status: 201 });
      }),
    );

    await duplicatePurpose(CUSTOM_ID, 'Dental');
    expect(sent).toEqual({ duplicateOf: CUSTOM_ID, name: 'Dental' });

    server.use(getCreatePurposeMockHandler(stored));
    server.use(getSavePurposeVersionMockHandler({ ...stored, latestVersion: 3 }));
    expect((await savePurposeVersion(CUSTOM_ID)).latestVersion).toBe(3);
  });

  it('reads the tool catalog', async () => {
    server.use(getListToolsMockHandler(toolCatalogBody()));

    const catalog = await listTools();

    expect(catalog.allowedHosts).toEqual(['calendar.example.com', '*.tools.test']);
    expect(catalog.builtins.find((builtin) => builtin.locked)?.name).toBe('mark_do_not_call');
  });

  it('reads field errors, details and a missing purpose', async () => {
    server.use(
      http.get('*/api/v1/purposes/:id', () =>
        problemResponse(422, [{ field: 'name', message: 'Name it.' }]),
      ),
    );
    const refused: unknown = await getPurpose(CUSTOM_ID).catch((caught: unknown) => caught);
    server.use(http.get('*/api/v1/purposes/:id', () => problemResponse(404)));
    const missing: unknown = await getPurpose(CUSTOM_ID).catch((caught: unknown) => caught);

    expect(fieldErrorsOf(refused)).toEqual([{ field: 'name', message: 'Name it.' }]);
    expect(fieldErrorsOf(missing)).toEqual([]);
    expect(problemDetail(missing)).toBe('The request failed.');
    expect(problemDetail(new Error('offline'))).toBeUndefined();
    expect(
      problemDetail(new ApiError(500, { type: 'about:blank', title: 'Error', status: 500 })),
    ).toBeUndefined();
    expect(isNotFound(missing)).toBe(true);
    expect(isNotFound(refused)).toBe(false);
  });
});
