// A voice session over the media WebSocket (vak.media.v1): microphone frames out, agent audio
// and events in, barge-in honoured within a frame. The browser parts come in through
// `VoiceDeps`, so tests drive the whole flow with fakes.
import { FrameAccumulator, pcm16ToFloat } from './audio-frames';
import {
  encodeClientEvent,
  MEDIA_PROTOCOL,
  parseServerEvent,
  type ClientEvent,
  type ServerEvent,
} from './media-protocol';
import { PlaybackQueue } from './playback';
import { openMicrophone, openSpeaker, type Microphone, type Speaker } from './web-audio';

/** The parts of a WebSocket a session uses. */
interface MediaSocket {
  binaryType: BinaryType;
  readonly readyState: number;
  addEventListener(type: 'open' | 'close', listener: () => void): void;
  addEventListener(type: 'message', listener: (event: MessageEvent<unknown>) => void): void;
  send(data: string | ArrayBuffer): void;
  close(): void;
}

export interface VoiceDeps {
  openSocket(url: string): MediaSocket;
  openMicrophone(onSamples: (samples: Float32Array) => void): Promise<Microphone>;
  openSpeaker(): Speaker;
  /** The page the media path is resolved against. */
  readonly page: string;
}

export interface VoiceSessionHandlers {
  /** Every server event, in order. */
  onEvent(event: ServerEvent): void;
  /** The socket closed: the session is over. */
  onClosed(): void;
}

export interface VoiceSession {
  setMuted(muted: boolean): void;
  /** Hangs up: the server answers `session.ended` and closes the socket. */
  end(): void;
}

const OPEN = 1;
/** How long a hang-up waits for the server to close before the client does. */
export const CLOSE_GRACE_MS = 2_000;

function browserDeps(): VoiceDeps {
  return {
    openSocket: (url) => new WebSocket(url, MEDIA_PROTOCOL),
    openMicrophone,
    openSpeaker,
    page: window.location.href,
  };
}

/** The WebSocket URL of a media path such as `/api/v1/sessions/{id}/media`. */
export function mediaSocketUrl(mediaPath: string, page: string): string {
  const url = new URL(mediaPath, page);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  return url.toString();
}

/**
 * Opens the speaker, the microphone and the media socket, and starts the session. Call it from
 * a click: browsers only let an audio context play after a user gesture.
 */
export async function openVoiceSession(
  mediaPath: string,
  handlers: VoiceSessionHandlers,
  deps: VoiceDeps = browserDeps(),
): Promise<VoiceSession> {
  const speaker = deps.openSpeaker();
  const queue = new PlaybackQueue(speaker);
  const frames = new FrameAccumulator();
  let socket: MediaSocket | null = null;
  let started = false;
  let muted = false;
  let closed = false;
  let hangUp: ReturnType<typeof setTimeout> | undefined;

  const sendEvent = (event: ClientEvent) => {
    if (socket?.readyState === OPEN) socket.send(encodeClientEvent(event));
  };

  let microphone: Microphone;
  try {
    microphone = await deps.openMicrophone((samples) => {
      if (!started || muted || socket?.readyState !== OPEN) return;
      for (const frame of frames.push(samples)) socket.send(frame);
    });
  } catch (error) {
    speaker.close();
    throw error;
  }

  const ws = deps.openSocket(mediaSocketUrl(mediaPath, deps.page));
  socket = ws;
  ws.binaryType = 'arraybuffer';
  ws.addEventListener('open', () => {
    sendEvent({ type: 'session.start' });
    started = true;
  });
  ws.addEventListener('message', ({ data }) => {
    if (data instanceof ArrayBuffer) {
      queue.push(pcm16ToFloat(data));
      return;
    }
    if (typeof data !== 'string') return;
    const event = parseServerEvent(data);
    if (event === null) return;
    if (event.type === 'agent.audio.start') queue.startTurn();
    if (event.type === 'agent.interrupted') queue.interrupt();
    handlers.onEvent(event);
  });
  ws.addEventListener('close', () => {
    if (closed) return;
    closed = true;
    clearTimeout(hangUp);
    microphone.close();
    queue.stop();
    speaker.close();
    handlers.onClosed();
  });

  return {
    setMuted(next) {
      muted = next;
      sendEvent({ type: 'client.mute', muted: next });
    },
    end() {
      if (closed || hangUp !== undefined) return;
      if (ws.readyState !== OPEN) {
        ws.close();
        return;
      }
      sendEvent({ type: 'session.end' });
      hangUp = setTimeout(() => {
        ws.close();
      }, CLOSE_GRACE_MS);
    },
  };
}
