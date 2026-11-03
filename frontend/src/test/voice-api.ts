// An MSW fake of the voice endpoints (start a session, latency histograms), shaped like
// contracts/openapi.json, for tests outside features/*/api.
import { HttpResponse, http } from 'msw';

export const SESSION_ID = '01920000-0000-7000-8000-0000000000aa';
export const MEDIA_PATH = `/api/v1/sessions/${SESSION_ID}/media`;

const STAGES = ['stt', 'decision', 'llm_ttft', 'tts_first_audio', 'mouth_to_ear'] as const;
const BOUNDS = [100, 200, 400, 800, null] as const;

/** Every stage with no replies yet, as a fresh server reports it. */
export function emptyLatencyBody() {
  return {
    stages: STAGES.map((stage) => ({
      stage,
      count: 0,
      p50Ms: null,
      p95Ms: null,
      buckets: BOUNDS.map((leMs) => ({ leMs, count: 0 })),
    })),
  };
}

/** Two replies measured: end to end took 350 ms and 900 ms. */
export function measuredLatencyBody() {
  const body = emptyLatencyBody();
  const endToEnd = body.stages.find((stage) => stage.stage === 'mouth_to_ear');
  if (endToEnd !== undefined) {
    Object.assign(endToEnd, {
      count: 2,
      p50Ms: 350,
      p95Ms: 900,
      buckets: BOUNDS.map((leMs) => ({ leMs, count: leMs === 400 || leMs === null ? 1 : 0 })),
    });
  }
  return body;
}

/** Handlers for a server with voice on; `latency` is what GET /voice/latency answers. */
export function fakeVoiceApi(latency: object = emptyLatencyBody()) {
  const started: string[] = [];
  const handlers = [
    http.post('*/api/v1/agents/:id/sessions', ({ params }) => {
      started.push(String(params['id']));
      return HttpResponse.json({ id: SESSION_ID, mediaUrl: MEDIA_PATH }, { status: 201 });
    }),
    http.get('*/api/v1/voice/latency', () => HttpResponse.json(latency)),
  ];
  return { handlers, started };
}
