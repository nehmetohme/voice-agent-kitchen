// The test call panel through the user's eyes, against the voice API fake and a fake browser:
// the real session code runs, only the socket and Web Audio are pretend.
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { HttpResponse, http, type RequestHandler } from 'msw';
import { describe, expect, it } from 'vitest';

import { AGENT_ID } from '../../../test/agents-api';
import { installFakeVoiceBrowser, lastSocket, microphone } from '../../../test/fake-voice-browser';
import { startMswServer } from '../../../test/msw-server';
import { renderRoutes } from '../../../test/render-routes';
import { emptyLatencyBody, fakeVoiceApi, measuredLatencyBody } from '../../../test/voice-api';
import { TestCallPanel } from './test-call-panel';

installFakeVoiceBrowser();
const server = startMswServer();

const format = (sampleRate: number) => ({
  encoding: 'pcm_s16le',
  sample_rate: sampleRate,
  frame_ms: 20,
});

/**
 * Renders the panel on the agent's page (its summary links to the Test step); `overrides`
 * answer before the fake (a failing endpoint, say).
 */
async function renderPanel(latency: object = emptyLatencyBody(), ...overrides: RequestHandler[]) {
  const api = fakeVoiceApi(latency);
  server.use(...overrides, ...api.handlers);
  renderRoutes(`/agents/${AGENT_ID}/provision`, [
    { path: '/agents/$agentId/provision', component: () => <TestCallPanel agentId={AGENT_ID} /> },
    { path: '/agents/$agentId/test', component: () => <p>Test step</p> },
  ]);
  await screen.findByRole('region', { name: 'Test call' });
  return api;
}

/** Waits for the page to open the media socket, then accepts it. */
async function answer() {
  await screen.findByText('Connecting…');
  const socket = await waitFor(() => lastSocket());
  act(() => {
    socket.open();
    socket.receive({
      type: 'session.started',
      session_id: 'abc',
      audio: { input: format(16_000), output: format(24_000) },
    });
  });
  return socket;
}

function transcriptLines() {
  const transcript = screen.getByRole('region', { name: 'Transcript' });
  return within(transcript)
    .getAllByRole('listitem')
    .map((item) => item.textContent);
}

describe('Test call', () => {
  it('starts a call, shows the conversation and hangs up', async () => {
    const api = await renderPanel();
    const section = screen.getByRole('region', { name: 'Test call' });
    expect(within(section).getByText('Talk to this agent through your microphone.')).toBeVisible();

    const start = within(section).getByRole('button', { name: 'Start test call' });
    expect(start).toHaveAttribute('data-variant', 'primary');
    fireEvent.click(start);
    const socket = await answer();
    expect(api.started).toEqual([AGENT_ID]);
    expect(await within(section).findByText('Live')).toBeVisible();
    expect(within(section).getByText('Listening')).toBeVisible();

    act(() => {
      socket.receive({ type: 'agent.audio.start', turn_id: 0 });
    });
    expect(within(section).getByText('Agent speaking')).toBeVisible();
    expect(transcriptLines()).toEqual(['Agent:Speaking…']);
    act(() => {
      socket.receive({
        type: 'transcript.final',
        turn_id: 0,
        speaker: 'agent',
        text: 'Hi, this is Front desk, an automated AI assistant.',
        interrupted: false,
      });
      socket.receive({ type: 'agent.audio.end', turn_id: 0 });
      socket.receive({ type: 'transcript.partial', turn_id: 1, speaker: 'caller', text: 'Book' });
    });
    expect(transcriptLines()).toEqual([
      'Agent:Hi, this is Front desk, an automated AI assistant.',
      'You:Book…',
    ]);

    act(() => {
      socket.receive({
        type: 'transcript.final',
        turn_id: 1,
        speaker: 'caller',
        text: 'Book a table.',
        interrupted: false,
      });
      socket.receive({ type: 'agent.audio.start', turn_id: 2 });
      socket.receive({
        type: 'latency',
        turn_id: 2,
        stt_ms: 90,
        llm_ttft_ms: 210,
        tts_first_audio_ms: 120,
        mouth_to_ear_ms: 640,
      });
      socket.receive({ type: 'agent.audio.end', turn_id: 2 });
      socket.receive({
        type: 'transcript.final',
        turn_id: 2,
        speaker: 'agent',
        text: 'For when?',
        interrupted: false,
      });
    });
    expect(transcriptLines().slice(1)).toEqual([
      'You:Book a table.',
      'Agent · replied in 640 ms:For when?',
    ]);
    expect(within(section).getByText('Listening')).toBeVisible();

    const mute = within(section).getByRole('button', { name: 'Mute' });
    fireEvent.click(mute);
    expect(mute).toHaveAttribute('aria-pressed', 'true');
    expect(socket.events()).toContainEqual({ type: 'client.mute', muted: true });

    fireEvent.click(within(section).getByRole('button', { name: 'End call' }));
    expect(within(section).getByText('Hanging up…')).toBeVisible();
    expect(socket.events().at(-1)).toEqual({ type: 'session.end' });
    act(() => {
      socket.receive({ type: 'session.ended', reason: 'client_ended' });
      socket.close();
    });
    expect(within(section).getByText('Call ended.')).toBeVisible();
    expect(within(section).getByRole('button', { name: 'Call again' })).toBeVisible();
    expect(microphone.track.stop).toHaveBeenCalledOnce();

    // The summary (peak-end): turns and this call's reply latency, then the next steps.
    const summary = within(section).getByRole('status', { name: 'Call summary' });
    expect(summary).toHaveTextContent('3 turns · replies p50 640 ms · p95 640 ms');
    expect(within(section).getByRole('button', { name: 'Publish' })).toBeDisabled();
    expect(within(section).getByText('Later')).toBeVisible();
    fireEvent.click(within(section).getByRole('link', { name: 'Run tests' }));
    expect(await screen.findByText('Test step')).toBeVisible();
  });

  it('shows a caller line spoken over the disclosure after it, and marks an interruption', async () => {
    await renderPanel();
    fireEvent.click(screen.getByRole('button', { name: 'Start test call' }));
    const socket = await answer();

    act(() => {
      socket.receive({ type: 'agent.audio.start', turn_id: 0 });
      socket.receive({
        type: 'transcript.final',
        turn_id: 1,
        speaker: 'caller',
        text: 'Okay.',
        interrupted: false,
      });
      socket.receive({ type: 'agent.interrupted', turn_id: 0 });
      socket.receive({
        type: 'transcript.final',
        turn_id: 0,
        speaker: 'agent',
        text: 'Hi, this is',
        interrupted: true,
      });
    });

    expect(transcriptLines()).toEqual(['Agent:Hi, this is (interrupted)', 'You:Okay.']);
  });

  it('says why the agent ended the call', async () => {
    await renderPanel();

    fireEvent.click(screen.getByRole('button', { name: 'Start test call' }));
    const socket = await answer();
    act(() => {
      socket.receive({ type: 'error', code: 'provider_unavailable', message: 'timed out' });
    });
    expect(screen.getByRole('alert')).toHaveTextContent("A model didn't answer in time.");
    act(() => {
      socket.receive({ type: 'session.ended', reason: 'opt_out', opt_out_layer: 'phrase' });
      socket.close();
    });

    expect(screen.getByText('The caller opted out, so the agent ended the call.')).toBeVisible();
    expect(screen.queryByRole('alert')).toBeNull();
  });

  it('shows the server reason when a call is refused', async () => {
    await renderPanel(
      emptyLatencyBody(),
      http.post('*/api/v1/agents/:id/sessions', () =>
        HttpResponse.json(
          {
            title: 'Unavailable',
            status: 503,
            detail: "Voice sessions aren't set up on this server.",
          },
          { status: 503, headers: { 'content-type': 'application/problem+json' } },
        ),
      ),
    );

    fireEvent.click(screen.getByRole('button', { name: 'Start test call' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      "Voice sessions aren't set up on this server.",
    );
    expect(screen.getByRole('button', { name: 'Call again' })).toBeVisible();
  });

  it('asks for the microphone when the browser refuses it', async () => {
    await renderPanel();
    microphone.getUserMedia.mockRejectedValue(new DOMException('denied', 'NotAllowedError'));

    fireEvent.click(screen.getByRole('button', { name: 'Start test call' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Allow microphone access');
  });

  it('never claims the page status role, which the autosave status owns', async () => {
    await renderPanel();

    expect(screen.queryByRole('status')).toBeNull();
  });
});

describe('Latency', () => {
  it('shows each stage median in one row, and the full table in Details', async () => {
    await renderPanel(measuredLatencyBody());

    const strip = screen.getByRole('region', { name: 'Latency' });
    expect(await within(strip).findByText('End to end')).toBeVisible();
    expect(within(strip).getByText('End to end').nextSibling).toHaveTextContent(
      '350 ms, 95th percentile 900 ms',
    );
    expect(within(strip).getByText('Speech to text').nextSibling).toHaveTextContent('–');
    expect(screen.queryByRole('table')).toBeNull();

    fireEvent.click(within(strip).getByRole('button', { name: 'Latency details' }));
    const details = await screen.findByRole('dialog', { name: 'Latency' });
    const rows = within(details).getAllByRole('row');
    expect(rows.map((row) => row.textContent)).toEqual([
      'StageMedian95th percentileRepliesSpread',
      'Speech to text––0No replies yet',
      'Opt-out check––0No replies yet',
      'First token––0No replies yet',
      'First audio––0No replies yet',
      'End to end350 ms900 ms2Replies by time, ≤ 400 ms: 1, > 800 ms: 1',
    ]);
  });

  it('offers a retry when latency fails to load', async () => {
    await renderPanel(
      emptyLatencyBody(),
      http.get('*/api/v1/voice/latency', () => HttpResponse.error(), { once: true }),
    );

    expect(await screen.findByRole('alert')).toHaveTextContent("Couldn't load latency");
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));

    expect(await screen.findByText('End to end')).toBeVisible();
    expect(screen.getByRole('button', { name: 'Latency details' })).toBeVisible();
    expect(screen.queryByRole('alert')).toBeNull();
  });
});
