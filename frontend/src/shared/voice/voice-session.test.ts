// The voice session end to end in jsdom: the real capture, playback and socket code against a
// fake browser (src/test/fake-voice-browser.ts).
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  FakeAudioContext,
  FakeSocket,
  installFakeVoiceBrowser,
  lastSocket,
  microphone,
} from '../../test/fake-voice-browser';
import type { ServerEvent } from './media-protocol';
import { CLOSE_GRACE_MS, mediaSocketUrl, openVoiceSession } from './voice-session';

installFakeVoiceBrowser();

const MEDIA_PATH = '/api/v1/sessions/0192/media';
const format = (sampleRate: number) => ({
  encoding: 'pcm_s16le',
  sample_rate: sampleRate,
  frame_ms: 20,
});
const started = {
  type: 'session.started',
  session_id: 'abc',
  audio: { input: format(16_000), output: format(24_000) },
};
const quantum = () => new Float32Array(128).fill(0.25);

async function open() {
  const events: ServerEvent[] = [];
  const onClosed = vi.fn<() => void>();
  const session = await openVoiceSession(MEDIA_PATH, {
    onEvent: (event) => events.push(event),
    onClosed,
  });
  return { session, events, onClosed, socket: lastSocket() };
}

function contextAt(sampleRate: number): FakeAudioContext {
  const context = FakeAudioContext.instances.find((each) => each.sampleRate === sampleRate);
  if (context === undefined) throw new Error(`no ${sampleRate} Hz context`);
  return context;
}

afterEach(() => {
  vi.useRealTimers();
});

describe('openVoiceSession', () => {
  it('opens the media socket with the subprotocol and echo cancellation on', async () => {
    const { socket } = await open();

    const url = new URL(socket.url);
    expect(url.protocol).toBe('ws:');
    expect(url.pathname).toBe(MEDIA_PATH);
    expect(socket.protocol).toBe('vak.media.v1');
    expect(socket.binaryType).toBe('arraybuffer');
    expect(microphone.getUserMedia).toHaveBeenCalledWith({
      audio: expect.objectContaining({ echoCancellation: true, channelCount: 1 }) as unknown,
    });
    expect(contextAt(16_000).audioWorklet.addModule).toHaveBeenCalledWith(
      expect.stringMatching(/^blob:/) as unknown,
    );
  });

  it('sends session.start first, then 20 ms microphone frames', async () => {
    const { socket } = await open();

    microphone.speak(new Float32Array(640));
    expect(socket.sent).toHaveLength(0);
    socket.open();
    microphone.speak(quantum());
    microphone.speak(quantum());
    microphone.speak(quantum());

    expect(socket.events()).toEqual([{ type: 'session.start' }]);
    expect(socket.audioFrames().map((frame) => frame.byteLength)).toEqual([640]);
  });

  it('hands every valid server event on, and ignores anything else', async () => {
    const { socket, events } = await open();
    socket.open();

    socket.receive(started);
    socket.receive({ type: 'surprise' });
    socket.dispatchEvent(new MessageEvent('message', { data: 'not json' }));
    socket.dispatchEvent(new MessageEvent('message', { data: 42 }));

    expect(events).toEqual([started]);
  });

  it('plays the agent and stops it at once on barge-in', async () => {
    const { socket } = await open();
    socket.open();
    const speaker = contextAt(24_000);

    socket.receive(new ArrayBuffer(960)); // before agent.audio.start: dropped
    socket.receive({ type: 'agent.audio.start', turn_id: 1 });
    socket.receive(new ArrayBuffer(960));
    socket.receive(new ArrayBuffer(960));
    expect(speaker.sources).toHaveLength(2);
    expect(speaker.sources[0]?.start).toHaveBeenCalledWith(0.06);
    expect(speaker.sources[1]?.start).toHaveBeenCalledWith(0.08);

    socket.receive({ type: 'agent.interrupted', turn_id: 1 });
    for (const source of speaker.sources) expect(source.stop).toHaveBeenCalledOnce();
    socket.receive(new ArrayBuffer(960));
    expect(speaker.sources).toHaveLength(2);
  });

  it('stops sending audio while muted, and tells the server', async () => {
    const { session, socket } = await open();
    socket.open();

    session.setMuted(true);
    microphone.speak(new Float32Array(640));
    session.setMuted(false);
    microphone.speak(new Float32Array(320));

    expect(socket.events()).toEqual([
      { type: 'session.start' },
      { type: 'client.mute', muted: true },
      { type: 'client.mute', muted: false },
    ]);
    expect(socket.audioFrames()).toHaveLength(1);
  });

  it('hangs up with session.end and cleans up when the socket closes', async () => {
    const { session, socket, onClosed } = await open();
    socket.open();

    session.end();
    session.end();
    expect(socket.events().at(-1)).toEqual({ type: 'session.end' });
    socket.close();

    expect(onClosed).toHaveBeenCalledOnce();
    expect(microphone.track.stop).toHaveBeenCalledOnce();
    expect(contextAt(16_000).close).toHaveBeenCalledOnce();
    expect(contextAt(24_000).close).toHaveBeenCalledOnce();
    session.end();
    // Ending twice, or after the close, sends nothing more.
    expect(socket.events()).toEqual([{ type: 'session.start' }, { type: 'session.end' }]);
  });

  it('closes the socket itself if the server does not, after a grace period', async () => {
    vi.useFakeTimers();
    const { session, socket, onClosed } = await open();
    socket.open();

    session.end();
    vi.advanceTimersByTime(CLOSE_GRACE_MS - 1);
    expect(onClosed).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);

    expect(onClosed).toHaveBeenCalledOnce();
  });

  it('just closes a socket that never opened', async () => {
    const { session, socket, onClosed } = await open();

    session.end();

    expect(socket.sent).toHaveLength(0);
    expect(onClosed).toHaveBeenCalledOnce();
  });

  it('fails without a socket when the microphone is refused', async () => {
    microphone.getUserMedia.mockRejectedValue(new DOMException('denied', 'NotAllowedError'));

    await expect(open()).rejects.toThrow('denied');

    expect(FakeSocket.instances).toHaveLength(0);
    expect(contextAt(24_000).close).toHaveBeenCalledOnce();
  });
});

describe('mediaSocketUrl', () => {
  it('follows the page: ws on http, wss on https', () => {
    expect(mediaSocketUrl(MEDIA_PATH, 'http://127.0.0.1:5173/agents')).toBe(
      `ws://127.0.0.1:5173${MEDIA_PATH}`,
    );
    expect(mediaSocketUrl(MEDIA_PATH, 'https://vak.example.com/agents/x')).toBe(
      `wss://vak.example.com${MEDIA_PATH}`,
    );
  });
});
