// A test call as the page shows it: a pure reducer over the session's events, so every
// transition is unit-tested without a socket or a microphone.
import type { ServerEvent, SessionEnded } from '../../../shared/voice/media-protocol';
import type { StartFailure } from '../api/voice-api';

export type CallPhase = 'idle' | 'connecting' | 'live' | 'ending' | 'ended' | 'failed';

/** Why a call stopped before it got going. */
export type CallFailure = StartFailure | { kind: 'microphone' };

export type EndReason = SessionEnded['reason'];
export type ServerErrorCode = Extract<ServerEvent, { type: 'error' }>['code'];

export interface TranscriptLine {
  /** Unique within a call: the line's position. */
  key: string;
  speaker: 'caller' | 'agent';
  text: string;
  /** The caller cut the agent off mid-sentence. */
  interrupted: boolean;
}

export interface TestCallState {
  phase: CallPhase;
  muted: boolean;
  agentSpeaking: boolean;
  lines: TranscriptLine[];
  /** What the caller is saying right now, before the final transcript. */
  hearing: string | null;
  /** Mouth-to-ear time of the last reply, in milliseconds. */
  lastReplyMs: number | null;
  endReason: EndReason | null;
  failure: CallFailure | null;
  /** The last problem the server reported during the call. */
  serverError: ServerErrorCode | null;
}

export type TestCallAction =
  | { type: 'start' }
  | { type: 'event'; event: ServerEvent }
  | { type: 'mute'; muted: boolean }
  | { type: 'hang-up' }
  | { type: 'closed' }
  | { type: 'failed'; failure: CallFailure };

export const initialCall: TestCallState = {
  phase: 'idle',
  muted: false,
  agentSpeaking: false,
  lines: [],
  hearing: null,
  lastReplyMs: null,
  endReason: null,
  failure: null,
  serverError: null,
};

/** Each server event by its type, so a handler gets exactly its own event. */
type EventOf = { [E in ServerEvent as E['type']]: E };
type Handlers = {
  [K in keyof EventOf]?: (state: TestCallState, event: EventOf[K]) => TestCallState;
};

const onEvent: Handlers = {
  'session.started': (state) => ({ ...state, phase: 'live' }),
  'transcript.partial': (state, event) => ({ ...state, hearing: event.text }),
  'transcript.final': (state, event) => ({
    ...state,
    hearing: event.speaker === 'caller' ? null : state.hearing,
    lines: [
      ...state.lines,
      {
        key: `line-${state.lines.length}`,
        speaker: event.speaker,
        text: event.text,
        interrupted: event.interrupted,
      },
    ],
  }),
  'agent.audio.start': (state) => ({ ...state, agentSpeaking: true }),
  'agent.audio.end': (state) => ({ ...state, agentSpeaking: false }),
  'agent.interrupted': (state) => ({ ...state, agentSpeaking: false }),
  latency: (state, event) => ({
    ...state,
    lastReplyMs: event.mouth_to_ear_ms ?? state.lastReplyMs,
  }),
  error: (state, event) => ({ ...state, serverError: event.code }),
  'session.ended': (state, event) => ({
    ...state,
    phase: 'ended',
    agentSpeaking: false,
    endReason: event.reason,
  }),
};

function applyEvent<K extends keyof EventOf>(
  state: TestCallState,
  type: K,
  event: EventOf[K],
): TestCallState {
  const handle = onEvent[type];
  return handle === undefined ? state : handle(state, event);
}

const isOver = (phase: CallPhase) => phase === 'ended' || phase === 'failed';

export function testCallReducer(state: TestCallState, action: TestCallAction): TestCallState {
  switch (action.type) {
    case 'start':
      return { ...initialCall, phase: 'connecting' };
    case 'event':
      return isOver(state.phase) ? state : applyEvent(state, action.event.type, action.event);
    case 'mute':
      return { ...state, muted: action.muted };
    case 'hang-up':
      return isOver(state.phase) ? state : { ...state, phase: 'ending' };
    case 'closed':
      return isOver(state.phase)
        ? state
        : { ...state, phase: 'ended', agentSpeaking: false, hearing: null };
    case 'failed':
    default:
      return { ...state, phase: 'failed', agentSpeaking: false, failure: action.failure };
  }
}

/** Whether a call is under way (the page offers "End call" rather than "Start"). */
export const isInCall = (phase: CallPhase) =>
  phase === 'connecting' || phase === 'live' || phase === 'ending';
