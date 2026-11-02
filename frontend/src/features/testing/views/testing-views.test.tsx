// The test call and latency panels through the user's eyes, against the voice API fake and a
// fake browser: the real session code runs, only the socket and Web Audio are pretend.
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { HttpResponse, http, type RequestHandler } from 'msw';
import { describe, expect, it } from 'vitest';

import { AGENT_ID } from '../../../test/agents-api';
import { installFakeVoiceBrowser, lastSocket, microphone } from '../../../test/fake-voice-browser';
import { startMswServer } from '../../../test/msw-server';
import { emptyLatencyBody, fakeVoiceApi, measuredLatencyBody } from '../../../test/voice-api';
import { LatencyPanel } from './latency-panel';
import { TestCallPanel } from './test-call-panel';

installFakeVoiceBrowser();
const server = startMswServer();

const format = (sampleRate: number) => ({
  encoding: 'pcm_s16le',
  sample_rate: sampleRate,
  frame_ms: 20,
});

/** Renders both panels; `overrides` answer before the fake (a failing endpoint, say). */
function renderPanels(latency: object = emptyLatencyBody(), ...overrides: RequestHandler[]) {
  const api = fakeVoiceApi(latency);
  server.use(...overrides, ...api.handlers);
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <TestCallPanel agentId={AGENT_ID} />
      <LatencyPanel />
    </QueryClientProvider>,
  );
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

describe('Test call', () => {
  it('starts a call, shows the conversation and hangs up', async () => {
    const api = renderPanels();
    const section = screen.getByRole('region', { name: 'Test call' });

    fireEvent.click(within(section).getByRole('button', { name: 'Start test call' }));
    const socket = await answer();
    expect(api.started).toEqual([AGENT_ID]);
    expect(await within(section).findByText('Listening.')).toBeVisible();

    act(() => {
      socket.receive({ type: 'agent.audio.start', turn_id: 0 });
      socket.receive({
        type: 'transcript.final',
        turn_id: 0,
        speaker: 'agent',
        text: 'Hi, this is Front desk, an automated AI assistant.',
        interrupted: false,
      });
    });
    expect(within(section).getByText('Agent speaking.')).toBeVisible();
    act(() => {
      socket.receive({ type: 'agent.audio.end', turn_id: 0 });
      socket.receive({ type: 'transcript.partial', turn_id: 1, speaker: 'caller', text: 'Book' });
    });
    const transcript = within(section).getByRole('list', { name: 'Transcript' });
    expect(
      within(transcript)
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual(['Agent: Hi, this is Front desk, an automated AI assistant.', 'You: Book…']);

    act(() => {
      socket.receive({
        type: 'latency',
        turn_id: 1,
        stt_ms: 90,
        llm_ttft_ms: 210,
        tts_first_audio_ms: 120,
        mouth_to_ear_ms: 640,
      });
    });
    expect(within(section).getByText('Listening. Last reply took 640 ms.')).toBeVisible();

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
  });

  it('says why the agent ended the call', async () => {
    renderPanels();

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
    renderPanels(
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
    renderPanels();
    microphone.getUserMedia.mockRejectedValue(new DOMException('denied', 'NotAllowedError'));

    fireEvent.click(screen.getByRole('button', { name: 'Start test call' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Allow microphone access');
  });

  it('never claims the page status role, which the autosave status owns', () => {
    renderPanels();

    expect(screen.queryByRole('status')).toBeNull();
  });
});

describe('Latency', () => {
  it('shows each stage with its median, 95th percentile and spread', async () => {
    renderPanels(measuredLatencyBody());

    const table = await screen.findByRole('table');
    const rows = within(table).getAllByRole('row');
    expect(rows.map((row) => row.textContent)).toEqual([
      'StageMedian95th percentileRepliesSpread',
      'Speech to text––0No replies yet',
      'Model, first token––0No replies yet',
      'Voice, first audio––0No replies yet',
      'End to end350 ms900 ms2Replies by time, ≤ 400 ms: 1, > 800 ms: 1',
    ]);
  });

  it('offers a retry when latency fails to load', async () => {
    renderPanels(
      emptyLatencyBody(),
      http.get('*/api/v1/voice/latency', () => HttpResponse.error(), { once: true }),
    );

    expect(await screen.findByRole('alert')).toHaveTextContent("Couldn't load latency");
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));

    expect(await screen.findByRole('table')).toBeVisible();
    expect(screen.queryByRole('alert')).toBeNull();
  });
});
