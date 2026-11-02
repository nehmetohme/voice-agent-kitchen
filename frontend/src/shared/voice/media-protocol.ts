// The vak.media.v1 protocol, browser side: the text events of the media WebSocket
// (GET /api/v1/sessions/{id}/media). The schema and examples in contracts/media/ are the source
// of truth; media-protocol.contract.test.ts checks this module against them.

/** The WebSocket subprotocol. */
export const MEDIA_PROTOCOL = 'vak.media.v1';
/** Sample rate of the microphone audio the client sends (PCM s16le mono). */
export const INPUT_SAMPLE_RATE = 16_000;
/** Sample rate of the agent audio the server sends (PCM s16le mono). */
export const OUTPUT_SAMPLE_RATE = 24_000;
/** Length of one audio frame, in either direction. */
export const FRAME_MS = 20;

export type Speaker = 'caller' | 'agent';
export type EndReason =
  | 'client_ended'
  | 'opt_out'
  | 'escalation'
  | 'end_node'
  | 'timeout'
  | 'error';
export type OptOutLayer = 'phrase' | 'decision' | 'llm_tool';
export type ErrorCode = 'bad_message' | 'unsupported_audio' | 'provider_unavailable' | 'internal';

interface AudioFormat {
  encoding: 'pcm_s16le';
  sample_rate: number;
  frame_ms: number;
}

/** An event the server sends. */
export type ServerEvent =
  | {
      type: 'session.started';
      session_id: string;
      audio: { input: AudioFormat; output: AudioFormat };
    }
  | { type: 'transcript.partial'; turn_id: number; speaker: 'caller'; text: string }
  | {
      type: 'transcript.final';
      turn_id: number;
      speaker: Speaker;
      text: string;
      interrupted: boolean;
    }
  | { type: 'agent.audio.start'; turn_id: number }
  | { type: 'agent.audio.end'; turn_id: number }
  | { type: 'agent.interrupted'; turn_id: number }
  | { type: 'plan.node'; turn_id: number; node_id: string }
  | {
      type: 'decision';
      turn_id: number;
      question: string;
      status: 'answered' | 'timeout' | 'error';
      latency_ms: number;
      probability?: number;
      choice?: string;
    }
  | {
      type: 'latency';
      turn_id: number;
      stt_ms: number | null;
      llm_ttft_ms: number | null;
      tts_first_audio_ms: number | null;
      mouth_to_ear_ms: number | null;
    }
  | { type: 'error'; code: ErrorCode; message: string }
  | { type: 'session.ended'; reason: EndReason; opt_out_layer?: OptOutLayer };

export type ServerEventType = ServerEvent['type'];

/** An event the client sends. */
export type ClientEvent =
  | { type: 'session.start'; audio?: { encoding: 'pcm_s16le'; sample_rate: 16_000 } }
  | { type: 'client.mute'; muted: boolean }
  | { type: 'session.end' };

/** Every server event type this client understands. */
// CONTRACT-STUB
export const SERVER_EVENT_TYPES: readonly ServerEventType[] = [];

/**
 * Reads a text frame from the server. Returns `null` for anything that is not a valid
 * vak.media.v1 server event (unknown types included), so the caller can ignore it.
 */
export function parseServerEvent(data: string): ServerEvent | null {
  // CONTRACT-STUB
  void data;
  return null;
}

/** The text frame for a client event. */
export function encodeClientEvent(event: ClientEvent): string {
  // CONTRACT-STUB
  void event;
  return '';
}
