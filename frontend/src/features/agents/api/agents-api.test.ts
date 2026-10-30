import { HttpResponse, http } from 'msw';
import { describe, expect, it } from 'vitest';

import {
  getArchiveAgentMockHandler,
  getCreateAgentMockHandler,
  getGetAgentMockHandler,
  getGetAgentProgressMockHandler,
  getListAgentsMockHandler,
  getUpdateAgentMockHandler,
} from '../../../shared/api/generated/endpoints/agents/agents.msw';
import {
  getListLanguagesMockHandler,
  getListVoicesMockHandler,
} from '../../../shared/api/generated/endpoints/catalog/catalog.msw';
import { ApiError } from '../../../shared/api/http';
import {
  AGENT_ID,
  agentBody,
  languagesBody,
  problemResponse,
  progressBody,
  voicesBody,
} from '../../../test/agents-api';
import { startMswServer } from '../../../test/msw-server';
import {
  archiveAgent,
  createAgent,
  fieldErrorsOf,
  getAgent,
  getAgentProgress,
  isNotFound,
  listAgents,
  listLanguages,
  listVoices,
  updateAgent,
} from './agents-api';
import { LIMITS, checkSettings } from './request-checks';

const server = startMswServer();

const settings = {
  name: 'Front desk',
  useCase: 'support',
  voice: 'af_heart',
  language: 'en-US',
  jurisdiction: 'US',
  model: 'balanced',
  turnDetection: { silenceMs: 500, bargeIn: true },
};

describe('agents API', () => {
  it('lists agents as list items', async () => {
    server.use(
      getListAgentsMockHandler({
        items: [
          {
            id: AGENT_ID,
            name: 'Front desk',
            useCase: 'booking',
            voice: 'af_heart',
            updatedAt: '2026-10-30T10:00:00Z',
            progressPercent: 25,
          },
        ],
      }),
    );

    await expect(listAgents()).resolves.toEqual([
      {
        id: AGENT_ID,
        name: 'Front desk',
        useCase: 'booking',
        updatedAt: '2026-10-30T10:00:00Z',
        progressPercent: 25,
      },
    ]);
  });

  it('maps an agent to its settings and archived flag', async () => {
    server.use(getGetAgentMockHandler(agentBody({ archivedAt: '2026-10-30T11:00:00Z' })));

    await expect(getAgent(AGENT_ID)).resolves.toEqual({ id: AGENT_ID, archived: true, settings });
  });

  it('creates with the three required fields only', async () => {
    let sent: unknown;
    server.use(
      getCreateAgentMockHandler(async ({ request }) => {
        sent = await request.json();
        return agentBody();
      }),
    );

    const created = await createAgent({
      name: 'Front desk',
      useCase: 'support',
      voice: 'af_heart',
    });

    expect(sent).toEqual({ name: 'Front desk', useCase: 'support', voice: 'af_heart' });
    expect(created).toEqual({ id: AGENT_ID, archived: false, settings });
  });

  it('sends only the changed fields', async () => {
    let sent: unknown;
    server.use(
      getUpdateAgentMockHandler(async ({ request }) => {
        sent = await request.json();
        return agentBody({ name: 'After hours' });
      }),
    );

    const updated = await updateAgent(AGENT_ID, { name: 'After hours' });

    expect(sent).toEqual({ name: 'After hours' });
    expect(updated.settings.name).toBe('After hours');
  });

  it('archives', async () => {
    server.use(getArchiveAgentMockHandler());

    await expect(archiveAgent(AGENT_ID)).resolves.toBeUndefined();
  });

  it('reads setup progress', async () => {
    server.use(getGetAgentProgressMockHandler(progressBody()));

    await expect(getAgentProgress(AGENT_ID)).resolves.toEqual(progressBody());
  });

  it('turns the catalog into picker choices', async () => {
    server.use(
      getListVoicesMockHandler(voicesBody()),
      getListLanguagesMockHandler(languagesBody()),
    );

    await expect(listVoices()).resolves.toEqual([
      { value: 'af_heart', label: 'Heart · American English, female' },
      { value: 'bm_george', label: 'George · British English, male' },
    ]);
    await expect(listLanguages()).resolves.toEqual([
      { value: 'en-US', label: 'English (United States)' },
      { value: 'en-GB', label: 'English (United Kingdom)' },
    ]);
  });
});

describe('request checks (POS-4)', () => {
  it("takes the limits from the contract's request schema", () => {
    expect(LIMITS).toEqual({ nameMax: 80, silenceMinMs: 200, silenceMaxMs: 2000 });
  });

  it('names each field the schema rejects', () => {
    expect(checkSettings({ name: 'Front desk' })).toEqual({});
    expect(checkSettings({ name: '', turnDetection: { silenceMs: 50, bargeIn: true } })).toEqual({
      name: 'Enter a name of up to 80 characters.',
      turnDetection: 'Use a pause between 200 and 2000 ms.',
    });
  });

  it('never sends a body the schema rejects', async () => {
    let requests = 0;
    server.use(
      http.post('*/api/v1/agents', () => {
        requests += 1;
        return HttpResponse.json(agentBody(), { status: 201 });
      }),
      http.patch('*/api/v1/agents/:id', () => {
        requests += 1;
        return HttpResponse.json(agentBody());
      }),
    );

    await expect(createAgent({ name: '', useCase: 'support', voice: 'af_heart' })).rejects.toThrow(
      /too_small/,
    );
    await expect(updateAgent(AGENT_ID, { name: 'a'.repeat(81) })).rejects.toThrow(/too_big/);
    expect(requests).toBe(0);
  });
});

describe('API errors', () => {
  it('reads the field errors of a 422 problem and ignores unknown fields', async () => {
    server.use(
      http.patch('*/api/v1/agents/:id', () =>
        problemResponse(422, [
          { field: 'voice', message: 'Choose a voice from the catalog.' },
          { field: 'nonsense', message: 'Ignored.' },
        ]),
      ),
    );

    const error: unknown = await updateAgent(AGENT_ID, { voice: 'x' }).catch(
      (caught: unknown) => caught,
    );

    expect(fieldErrorsOf(error)).toEqual({ voice: 'Choose a voice from the catalog.' });
    expect(isNotFound(error)).toBe(false);
  });

  it('has no field errors for other failures', async () => {
    server.use(http.get('*/api/v1/agents/:id', () => problemResponse(404)));

    const error: unknown = await getAgent(AGENT_ID).catch((caught: unknown) => caught);

    expect(isNotFound(error)).toBe(true);
    expect(fieldErrorsOf(error)).toEqual({});
    expect(fieldErrorsOf(new Error('offline'))).toEqual({});
    expect(fieldErrorsOf(new ApiError(422, { status: 422 }))).toEqual({});
    expect(isNotFound(new Error('offline'))).toBe(false);
  });

  it('rejects a body that breaks the contract', async () => {
    server.use(http.get('*/api/v1/agents', () => HttpResponse.json({ items: [{ id: 1 }] })));

    await expect(listAgents()).rejects.toThrow('does not match the API contract');
  });
});
