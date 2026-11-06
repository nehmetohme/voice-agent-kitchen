// Golden path `test-call` (docs/ux-guardrails/golden-paths.md): talk to the agent in the
// browser. Start a test call, see the live transcript with a latency badge on each reply,
// interrupt the agent, end the call, and land on the summary (peak-end): turns, latency p50/p95,
// the transcript, and a "Publish" call to action. Feedback on "Start test call" within 100 ms.
// Part of contract test-runs v1 (pinned by contract/test-runs/v1): never edit it once tagged.
//
// Runs against the real Rust server and Postgres (agents, progress, latency). The e2e server has
// no speech models, so only the call itself is faked, as in the agent-workspace contract:
// `POST /agents/{id}/sessions` and its media WebSocket (vak.media.v1). Chromium's fake
// microphone plays a recorded utterance (backend/fixtures/audio), and the fake server checks
// that its audio arrives.
import { resolve } from 'node:path';

import {
  expect,
  test,
  type APIRequestContext,
  type Page,
  type WebSocketRoute,
} from '@playwright/test';

import { auditScreen } from '../support/ux-audit';

const FEEDBACK_BUDGET_MS = 100;
const UTTERANCE = resolve(
  import.meta.dirname,
  '../../../backend/fixtures/audio/utterance_book_table_16k.wav',
);
const SESSION_ID = '0192f4c1-7c1e-7d34-9a51-3c2b1d0e9f88';
const SESSION_STARTED = {
  type: 'session.started',
  session_id: SESSION_ID,
  audio: {
    input: { encoding: 'pcm_s16le', sample_rate: 16_000, frame_ms: 20 },
    output: { encoding: 'pcm_s16le', sample_rate: 24_000, frame_ms: 20 },
  },
};

async function createAgent(request: APIRequestContext, name: string): Promise<string> {
  const response = await request.post('/api/v1/agents', {
    data: { name, useCase: 'survey', voice: 'af_heart' },
  });
  expect(response.status()).toBe(201);
  const body: unknown = await response.json();
  const id: unknown = typeof body === 'object' && body !== null && 'id' in body ? body.id : null;
  if (typeof id !== 'string') throw new Error(`no agent id in ${JSON.stringify(body)}`);
  return id;
}

/** The fake server's side of the call: what it sends, and what it heard. */
interface FakeCall {
  send(event: object): void;
  /** Binary (microphone) frames received since the session started. */
  audioFrames(): number;
}

/** Fakes `POST /sessions` and the media socket; `session.end` is answered as the server would. */
async function fakeCall(page: Page): Promise<{ answered: Promise<FakeCall> }> {
  await page.route('**/api/v1/agents/*/sessions', (route) =>
    route.fulfill({
      status: 201,
      json: { id: SESSION_ID, mediaUrl: `/api/v1/sessions/${SESSION_ID}/media` },
    }),
  );
  let routed: Promise<void> = Promise.resolve();
  const answered = new Promise<FakeCall>((resolveCall) => {
    let frames = 0;
    let started = false;
    routed = page.routeWebSocket('**/api/v1/sessions/*/media', (socket: WebSocketRoute) => {
      const send = (event: object) => socket.send(JSON.stringify(event));
      socket.onMessage((message) => {
        if (typeof message !== 'string') {
          if (started) frames += 1;
          return;
        }
        const event: unknown = JSON.parse(message);
        const type = typeof event === 'object' && event !== null && 'type' in event && event.type;
        if (type === 'session.start') {
          started = true;
          send(SESSION_STARTED);
          resolveCall({ send, audioFrames: () => frames });
        }
        if (type === 'session.end') {
          send({ type: 'session.ended', reason: 'client_ended' });
        }
      });
    });
  });
  await routed;
  return { answered };
}

function agentLine(call: FakeCall, turn: number, text: string, interrupted = false): void {
  call.send({ type: 'transcript.final', turn_id: turn, speaker: 'agent', text, interrupted });
}

function callerLine(call: FakeCall, turn: number, text: string): void {
  call.send({
    type: 'transcript.final',
    turn_id: turn,
    speaker: 'caller',
    text,
    interrupted: false,
  });
}

function latency(call: FakeCall, turn: number, mouthToEarMs: number): void {
  call.send({
    type: 'latency',
    turn_id: turn,
    stt_ms: 120,
    llm_ttft_ms: 300,
    tts_first_audio_ms: 150,
    mouth_to_ear_ms: mouthToEarMs,
  });
}

/**
 * How long the page takes to show "Connecting…" after the next click, in ms. Call it before the
 * click and await it after: the listener is in place once the page has run this.
 */
function feedbackAfterNextClick(page: Page): Promise<number> {
  return page.evaluate(
    () =>
      new Promise<number>((done) => {
        document.addEventListener(
          'click',
          () => {
            const clicked = performance.now();
            if (document.body.textContent.includes('Connecting…')) {
              done(0);
              return;
            }
            const observer = new MutationObserver(() => {
              if (!document.body.textContent.includes('Connecting…')) return;
              observer.disconnect();
              done(performance.now() - clicked);
            });
            observer.observe(document.body, {
              subtree: true,
              childList: true,
              characterData: true,
            });
          },
          { once: true, capture: true },
        );
      }),
  );
}

// A fake microphone that plays a recorded utterance, never a real one.
test.use({
  permissions: ['microphone'],
  launchOptions: {
    args: [
      '--use-fake-ui-for-media-stream',
      '--use-fake-device-for-media-stream',
      `--use-file-for-fake-audio-capture=${UTTERANCE}`,
    ],
  },
});

test('talk to the agent and end on the summary', { tag: '@golden' }, async ({ page, request }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  const agentId = await createAgent(request, 'Golden desk');
  const { answered } = await fakeCall(page);
  await page.goto(`/agents/${agentId}/provision`);
  await expect(page.getByRole('status', { name: 'Save status' })).toHaveText('Saved');
  await auditScreen(page);

  // Start: the page answers the click at once.
  const feedback = feedbackAfterNextClick(page);
  await page.getByRole('button', { name: 'Start test call', exact: true }).click();
  expect(await feedback, 'feedback on Start within 100 ms').toBeLessThanOrEqual(FEEDBACK_BUDGET_MS);
  const call = await answered;
  await expect(page.getByRole('button', { name: 'End call', exact: true })).toBeEnabled();

  // The live transcript, with a latency badge on each reply.
  const transcript = page.getByRole('region', { name: 'Transcript', exact: true });
  const lines = transcript.getByRole('listitem');
  call.send({ type: 'agent.audio.start', turn_id: 0 });
  call.send({ type: 'agent.audio.end', turn_id: 0 });
  agentLine(
    call,
    0,
    'Hi, this is Golden desk, an automated AI assistant. This call may be recorded.',
  );
  callerLine(call, 1, "Hi, I'd like to take the survey.");
  call.send({ type: 'agent.audio.start', turn_id: 2 });
  latency(call, 2, 820);
  call.send({ type: 'agent.audio.end', turn_id: 2 });
  agentLine(call, 2, 'Great. How was your visit, from one to ten?');
  callerLine(call, 3, 'It was a nine.');
  await expect(lines).toHaveCount(4);
  await expect(lines.nth(2)).toContainText('820 ms');
  await auditScreen(page);

  // Interrupt the agent: speaking over it cuts the reply where it was.
  call.send({ type: 'agent.audio.start', turn_id: 4 });
  latency(call, 4, 1_240);
  await expect(lines.nth(4)).toContainText('1,240 ms');
  call.send({ type: 'agent.interrupted', turn_id: 4 });
  agentLine(call, 4, 'Thank you! Before you go,', true);
  callerLine(call, 5, 'Sorry, I have to run.');
  await expect(lines.nth(4)).toContainText('(interrupted)');
  await expect(lines).toHaveCount(6);
  expect(call.audioFrames(), "the microphone's audio reached the server").toBeGreaterThan(0);

  // End: the summary opens with the outcome and ends with the next step.
  await page.getByRole('button', { name: 'End call', exact: true }).click();
  const summary = page.getByRole('status', { name: 'Call summary', exact: true });
  await expect(summary).toContainText('6 turns');
  await expect(summary).toContainText('p50 820 ms');
  await expect(summary).toContainText('p95 1,240 ms');
  await expect(page.getByRole('button', { name: 'Publish', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Call again', exact: true })).toBeInViewport();
  await expect(lines).toHaveCount(6);
  await expect(lines.last()).toContainText('Sorry, I have to run.');
  const overflow = await page.evaluate(
    () => document.documentElement.scrollHeight - window.innerHeight,
  );
  expect(overflow, 'the summary fits without scrolling the page').toBeLessThanOrEqual(0);
  await auditScreen(page);
});
