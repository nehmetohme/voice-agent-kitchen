// Contract: agent workspace v1 (end to end). Pinned by the tag contract/agent-workspace/v1.
// Never edit this file; to change the contract, add a v2.
//
// The agent's page is a workspace that fits the screen, with the settings on one side and the
// test call on the other:
// - At 1280×800, 1440×900 and 1920×1080 the document never scrolls vertically. The settings
//   fields, setup progress, "Start test call" and the transcript are all in the viewport, the
//   settings left of the transcript; opening "Advanced" still doesn't scroll the page.
// - The transcript orders lines by turn, not by arrival: a caller line spoken over the agent's
//   disclosure (turn 0) shows after it, even when its final transcript arrives first.
// - The transcript scrolls inside its own region and follows the newest line. Scrolled up, it
//   stops following and offers "Jump to latest", which brings the newest line back and
//   follows again.
// - The end of a call says why it ended and offers "Call again", still without page scroll.
// - auditScreen passes, also during a call with a long transcript.
//
// Runs against the real Rust server and Postgres (agents, settings, progress, latency). The e2e
// server has no voice models, so only the call itself is faked: `POST /agents/{id}/sessions`
// and its media WebSocket (vak.media.v1), with Chromium's fake microphone.
import {
  expect,
  test,
  type APIRequestContext,
  type Locator,
  type Page,
  type WebSocketRoute,
} from '@playwright/test';

import { auditScreen } from '../../support/ux-audit';

const VIEWPORTS = [
  { width: 1280, height: 800 },
  { width: 1440, height: 900 },
  { width: 1920, height: 1080 },
] as const;

const SETTINGS_FIELDS = ['Name', 'Purpose', 'Voice', 'Language', 'Jurisdiction'] as const;

const SESSION_ID = '0192f4c1-7c1e-7d34-9a51-3c2b1d0e9f87';
const SESSION_STARTED = {
  type: 'session.started',
  session_id: SESSION_ID,
  audio: {
    input: { encoding: 'pcm_s16le', sample_rate: 16_000, frame_ms: 20 },
    output: { encoding: 'pcm_s16le', sample_rate: 24_000, frame_ms: 20 },
  },
};

/** One exchange after the disclosure: the agent asks (even turn), the caller answers (odd). */
const EXCHANGES = 20;

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

/** How far the document itself scrolls vertically; 0 when the page fits the viewport. */
function pageOverflow(page: Page): Promise<number> {
  return page.evaluate(() => document.documentElement.scrollHeight - window.innerHeight);
}

async function expectNoPageScroll(page: Page): Promise<void> {
  expect(await pageOverflow(page), 'the document must not scroll vertically').toBeLessThanOrEqual(
    0,
  );
}

async function boxOf(locator: Locator): Promise<{ x: number; width: number }> {
  const box = await locator.boundingBox();
  if (box === null) throw new Error(`${String(locator)} has no layout box`);
  return box;
}

function transcriptOf(page: Page): Locator {
  return page.getByRole('region', { name: 'Transcript', exact: true });
}

/** The page's media socket, as the fake server sees it. */
interface FakeCall {
  send(event: object): void;
}

/**
 * Fakes the voice call, the one thing the e2e server can't serve: starting a session answers a
 * media path, and the media socket answers `session.start` with `session.started`. Resolves
 * once the page has started the session; the test then speaks for the server.
 */
async function fakeCall(page: Page): Promise<{ answered: Promise<FakeCall> }> {
  await page.route('**/api/v1/agents/*/sessions', (route) =>
    route.fulfill({
      status: 201,
      json: { id: SESSION_ID, mediaUrl: `/api/v1/sessions/${SESSION_ID}/media` },
    }),
  );
  let routed: Promise<void> = Promise.resolve();
  const answered = new Promise<FakeCall>((resolve) => {
    routed = page.routeWebSocket('**/api/v1/sessions/*/media', (socket: WebSocketRoute) => {
      socket.onMessage((message) => {
        // Binary frames are the fake microphone; only text frames are events.
        if (typeof message !== 'string') return;
        const event: unknown = JSON.parse(message);
        const type = typeof event === 'object' && event !== null && 'type' in event && event.type;
        if (type !== 'session.start') return;
        socket.send(JSON.stringify(SESSION_STARTED));
        resolve({ send: (reply) => socket.send(JSON.stringify(reply)) });
      });
    });
  });
  await routed;
  return { answered };
}

/** One full agent turn: it starts speaking, finishes, and its final transcript arrives. */
function agentSays(call: FakeCall, turn: number, text: string): void {
  call.send({ type: 'agent.audio.start', turn_id: turn });
  call.send({ type: 'agent.audio.end', turn_id: turn });
  call.send({
    type: 'transcript.final',
    turn_id: turn,
    speaker: 'agent',
    text,
    interrupted: false,
  });
}

function callerSays(call: FakeCall, turn: number, text: string): void {
  call.send({
    type: 'transcript.final',
    turn_id: turn,
    speaker: 'caller',
    text,
    interrupted: false,
  });
}

// A fake microphone, never a real one (a worker option, so it is set for the whole file).
test.use({
  permissions: ['microphone'],
  launchOptions: {
    args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream'],
  },
});

test.describe('agent workspace v1', { tag: '@contract' }, () => {
  for (const viewport of VIEWPORTS) {
    test(`fits ${viewport.width}×${viewport.height} without scrolling the page`, async ({
      page,
      request,
    }) => {
      await page.setViewportSize(viewport);
      const agentId = await createAgent(request, `Workspace ${viewport.width}`);
      await page.goto(`/agents/${agentId}/provision`);
      await expect(page.getByRole('status', { name: 'Save status' })).toHaveText('Saved');
      await expect(page.getByLabel('Voice', { exact: true })).toBeEnabled();
      await expect(page.getByText('End to end', { exact: true })).toBeVisible();

      await expectNoPageScroll(page);
      await Promise.all(
        SETTINGS_FIELDS.map((label) =>
          expect(page.getByLabel(label, { exact: true })).toBeInViewport({ ratio: 1 }),
        ),
      );
      await expect(page.getByRole('progressbar', { name: 'Setup progress' })).toBeInViewport();
      const start = page.getByRole('button', { name: 'Start test call', exact: true });
      await expect(start).toBeInViewport({ ratio: 1 });
      const transcript = transcriptOf(page);
      await expect(transcript).toBeInViewport();
      const name = await boxOf(page.getByLabel('Name', { exact: true }));
      expect(name.x + name.width, 'settings sit left of the transcript').toBeLessThanOrEqual(
        (await boxOf(transcript)).x,
      );
      await auditScreen(page);

      // The advanced settings scroll inside the settings column, never the page.
      await page.getByText('Advanced', { exact: true }).click();
      await expect(page.getByLabel('Pause before replying (ms)', { exact: true })).toBeVisible();
      await expectNoPageScroll(page);
      await expect(start).toBeInViewport({ ratio: 1 });
      await auditScreen(page);
    });
  }

  test('orders the transcript by turn and follows the newest line', async ({ page, request }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    const agentId = await createAgent(request, 'Transcript desk');
    const { answered } = await fakeCall(page);
    await page.goto(`/agents/${agentId}/provision`);
    await page.getByRole('button', { name: 'Start test call', exact: true }).click();
    const call = await answered;
    await expect(page.getByRole('button', { name: 'End call', exact: true })).toBeEnabled();

    // The caller talks over the disclosure, and their final transcript arrives first.
    const transcript = transcriptOf(page);
    const lines = transcript.getByRole('listitem');
    call.send({ type: 'agent.audio.start', turn_id: 0 });
    callerSays(call, 1, 'Okay.');
    call.send({ type: 'agent.audio.end', turn_id: 0 });
    call.send({
      type: 'transcript.final',
      turn_id: 0,
      speaker: 'agent',
      text: 'Hi, this is Transcript desk, an automated AI assistant. This call may be recorded.',
      interrupted: false,
    });
    await expect(lines).toHaveText([/Agent.*Hi, this is Transcript desk/, /You.*Okay\./]);

    // A long conversation: the transcript scrolls by itself and keeps the newest line in view.
    for (let exchange = 1; exchange <= EXCHANGES; exchange += 1) {
      agentSays(call, exchange * 2, `Question ${exchange}: how would you rate our service today?`);
      callerSays(call, exchange * 2 + 1, `Answer ${exchange}: about a seven, thanks for asking.`);
    }
    await expect(lines).toHaveCount(2 + EXCHANGES * 2);
    await expect(lines.last()).toHaveText(new RegExp(`Answer ${EXCHANGES}:`));
    await expect(lines.last()).toBeInViewport();
    await expect(lines.first()).not.toBeInViewport();
    const jump = page.getByRole('button', { name: 'Jump to latest', exact: true });
    await expect(jump).toBeHidden();
    await expectNoPageScroll(page);
    await auditScreen(page);

    // Scrolled up, it stops following and offers a way back.
    await transcript.hover();
    await page.mouse.wheel(0, -5_000);
    await expect(jump).toBeVisible();
    await expect(lines.first()).toBeInViewport();
    callerSays(call, EXCHANGES * 2 + 2, 'One more thing.');
    await expect(lines).toHaveCount(3 + EXCHANGES * 2);
    await expect(lines.last()).not.toBeInViewport();
    await auditScreen(page);

    // Back at the newest line, it follows again.
    await jump.click();
    await expect(lines.last()).toBeInViewport();
    await expect(jump).toBeHidden();
    agentSays(call, EXCHANGES * 2 + 3, 'Of course. What is it?');
    await expect(lines).toHaveCount(4 + EXCHANGES * 2);
    await expect(lines.last()).toBeInViewport();

    // The call ends on why it ended, with the way to call again; the page still fits.
    call.send({ type: 'session.ended', reason: 'opt_out', opt_out_layer: 'phrase' });
    await expect(
      page.getByText('The caller opted out, so the agent ended the call.'),
    ).toBeVisible();
    await expect(page.getByRole('button', { name: 'Call again', exact: true })).toBeInViewport();
    await expectNoPageScroll(page);
    await auditScreen(page);
  });
});
