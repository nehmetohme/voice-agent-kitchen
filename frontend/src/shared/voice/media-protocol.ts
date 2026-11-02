// The vak.media.v1 protocol, browser side: the text events of the media WebSocket
// (GET /api/v1/sessions/{id}/media). The schema and examples in contracts/media/ are the source
// of truth; media-protocol.contract.test.ts checks this module against them.
import { z } from 'zod';

/** The WebSocket subprotocol. */
export const MEDIA_PROTOCOL = 'vak.media.v1';
/** Sample rate of the microphone audio the client sends (PCM s16le mono). */
export const INPUT_SAMPLE_RATE = 16_000;
/** Sample rate of the agent audio the server sends (PCM s16le mono). */
export const OUTPUT_SAMPLE_RATE = 24_000;
/** Length of one audio frame, in either direction. */
export const FRAME_MS = 20;

const turnId = z.number().int().min(0).max(4_294_967_295);
const milliseconds = z.number().int().min(0).max(4_294_967_295);
const text = z.string().min(1).max(10_000);
const audioFormat = (sampleRate: number) =>
  z.strictObject({
    encoding: z.literal('pcm_s16le'),
    sample_rate: z.literal(sampleRate),
    frame_ms: z.literal(FRAME_MS),
  });

const sessionStarted = z.strictObject({
  type: z.literal('session.started'),
  session_id: z.string().regex(/^[A-Za-z0-9_-]{1,64}$/),
  audio: z.strictObject({
    input: audioFormat(INPUT_SAMPLE_RATE),
    output: audioFormat(OUTPUT_SAMPLE_RATE),
  }),
});
const transcriptPartial = z.strictObject({
  type: z.literal('transcript.partial'),
  turn_id: turnId,
  speaker: z.literal('caller'),
  text,
});
const transcriptFinal = z.strictObject({
  type: z.literal('transcript.final'),
  turn_id: turnId,
  speaker: z.enum(['caller', 'agent']),
  text,
  interrupted: z.boolean(),
});
const turnEvent = <T extends string>(type: T) =>
  z.strictObject({ type: z.literal(type), turn_id: turnId });
const planNode = z.strictObject({
  type: z.literal('plan.node'),
  turn_id: turnId,
  node_id: z.string().min(1).max(128),
});
const decision = z
  .strictObject({
    type: z.literal('decision'),
    turn_id: turnId,
    question: z.string().regex(/^[a-z][a-z0-9_]{0,63}$/),
    status: z.enum(['answered', 'timeout', 'error']),
    latency_ms: milliseconds,
    probability: z.number().min(0).max(1).optional(),
    choice: z.string().min(1).max(128).optional(),
  })
  // An answer carries a probability or a choice; a timeout or error carries neither.
  .refine((event) =>
    event.status === 'answered'
      ? event.probability !== undefined || event.choice !== undefined
      : event.probability === undefined && event.choice === undefined,
  );
const stageMs = milliseconds.nullable();
const latency = z.strictObject({
  type: z.literal('latency'),
  turn_id: turnId,
  stt_ms: stageMs,
  llm_ttft_ms: stageMs,
  tts_first_audio_ms: stageMs,
  mouth_to_ear_ms: stageMs,
});
const error = z.strictObject({
  type: z.literal('error'),
  code: z.enum(['bad_message', 'unsupported_audio', 'provider_unavailable', 'internal']),
  message: z.string().min(1).max(500),
});
const sessionEnded = z
  .strictObject({
    type: z.literal('session.ended'),
    reason: z.enum(['client_ended', 'opt_out', 'escalation', 'end_node', 'timeout', 'error']),
    opt_out_layer: z.enum(['phrase', 'decision', 'llm_tool']).optional(),
  })
  // An opt-out names its layer; nothing else does.
  .refine((event) => (event.reason === 'opt_out') === (event.opt_out_layer !== undefined));

const serverEvent = z.union([
  sessionStarted,
  transcriptPartial,
  transcriptFinal,
  turnEvent('agent.audio.start'),
  turnEvent('agent.audio.end'),
  turnEvent('agent.interrupted'),
  planNode,
  decision,
  latency,
  error,
  sessionEnded,
]);

/** An event the server sends. */
export type ServerEvent = z.infer<typeof serverEvent>;
export type ServerEventType = ServerEvent['type'];
export type SessionEnded = z.infer<typeof sessionEnded>;

/** An event the client sends. */
export type ClientEvent =
  | { type: 'session.start'; audio?: { encoding: 'pcm_s16le'; sample_rate: 16_000 } }
  | { type: 'client.mute'; muted: boolean }
  | { type: 'session.end' };

/** Every server event type this client understands. */
export const SERVER_EVENT_TYPES: readonly ServerEventType[] = [
  'session.started',
  'transcript.partial',
  'transcript.final',
  'agent.audio.start',
  'agent.audio.end',
  'agent.interrupted',
  'plan.node',
  'decision',
  'latency',
  'error',
  'session.ended',
];

/**
 * Reads a text frame from the server. Returns `null` for anything that is not a valid
 * vak.media.v1 server event (unknown types included), so the caller can ignore it.
 */
export function parseServerEvent(data: string): ServerEvent | null {
  let json: unknown;
  try {
    json = JSON.parse(data);
  } catch {
    return null;
  }
  const parsed = serverEvent.safeParse(json);
  return parsed.success ? parsed.data : null;
}

/** The text frame for a client event. */
export function encodeClientEvent(event: ClientEvent): string {
  return JSON.stringify(event);
}
