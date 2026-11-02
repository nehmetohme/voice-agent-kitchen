import { HttpResponse, http } from 'msw';
import { describe, expect, it } from 'vitest';

import { getStartSessionMockHandler } from '../../../shared/api/generated/endpoints/sessions/sessions.msw';
import { getGetVoiceLatencyMockHandler } from '../../../shared/api/generated/endpoints/voice/voice.msw';
import { ApiError } from '../../../shared/api/http';
import { AGENT_ID, problemResponse } from '../../../test/agents-api';
import { startMswServer } from '../../../test/msw-server';
import { MEDIA_PATH, SESSION_ID } from '../../../test/voice-api';
import { getVoiceLatency, startFailureOf, startTestCall } from './voice-api';

const server = startMswServer();

describe('voice API', () => {
  it('starts a test call and returns its media path', async () => {
    server.use(getStartSessionMockHandler({ id: SESSION_ID, mediaUrl: MEDIA_PATH }));

    await expect(startTestCall(AGENT_ID)).resolves.toEqual({ mediaPath: MEDIA_PATH });
  });

  it('reads the latency histograms, with null for what was never measured', async () => {
    server.use(
      getGetVoiceLatencyMockHandler({
        stages: [
          {
            stage: 'mouth_to_ear',
            count: 2,
            p50Ms: 350,
            p95Ms: 900,
            buckets: [
              { leMs: 400, count: 1 },
              { leMs: null, count: 1 },
            ],
          },
          { stage: 'stt', count: 0, buckets: [{ count: 0 }] },
        ],
      }),
    );

    await expect(getVoiceLatency()).resolves.toEqual([
      {
        stage: 'mouth_to_ear',
        count: 2,
        p50Ms: 350,
        p95Ms: 900,
        buckets: [
          { leMs: 400, count: 1 },
          { leMs: null, count: 1 },
        ],
      },
      { stage: 'stt', count: 0, p50Ms: null, p95Ms: null, buckets: [{ leMs: null, count: 0 }] },
    ]);
  });

  it('reads why a call could not start', async () => {
    const refusal = (status: number, detail?: string) => {
      server.use(
        http.post('*/api/v1/agents/:id/sessions', () =>
          HttpResponse.json(
            { type: 'about:blank', title: 'Unavailable', status, detail },
            { status, headers: { 'content-type': 'application/problem+json' } },
          ),
        ),
      );
      return startTestCall(AGENT_ID).catch((error: unknown) => startFailureOf(error));
    };

    await expect(refusal(503, 'Too many test calls are running.')).resolves.toEqual({
      kind: 'refused',
      detail: 'Too many test calls are running.',
    });
    await expect(refusal(503)).resolves.toEqual({ kind: 'connection' });
    await expect(refusal(500, 'boom')).resolves.toEqual({ kind: 'connection' });

    server.use(http.post('*/api/v1/agents/:id/sessions', () => problemResponse(404)));
    await expect(startTestCall(AGENT_ID).catch(startFailureOf)).resolves.toEqual({
      kind: 'missing',
    });
    expect(startFailureOf(new ApiError(409, {}))).toEqual({ kind: 'missing' });
    expect(startFailureOf(new TypeError('Failed to fetch'))).toEqual({ kind: 'connection' });
  });
});
