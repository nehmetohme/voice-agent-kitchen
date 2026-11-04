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
  /** Unique within a call: the speaker and the turn. */
  key: string;
  /**
   * The turn it belongs to (vak.media.v1: 0 is the disclosure, then commit order). Lines show
   * in turn order, not arrival order: an agent turn's text only arrives once it is spoken.
   */
  turn: number;
  speaker: 'caller' | 'agent';
  /** `null` while the agent is still speaking the turn; its text comes when it finishes. */
  text: string | null;
  /** The caller cut the agent off mid-sentence. */
  interrupted: boolean;
  /** An agent reply's end-to-end time (end of your speech to its first sound), in ms. */
  replyMs: number | null;
}

export interface TestCallState {
  phase: CallPhase;
  muted: boolean;
  agentSpeaking: boolean;
  lines: TranscriptLine[];
  /** What the caller is saying right now, before the final transcript. */
  hearing: string | null;
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
  endReason: null,
  failure: null,
  serverError: null,
};

const keyOf = (speaker: TranscriptLine['speaker'], turn: number) => `${speaker}-${turn}`;

/**
 * Puts `line` in turn order. A line with the same key (the agent's reserved turn) is replaced,
 * keeping the reply time it already has.
 */
function place(lines: readonly TranscriptLine[], line: TranscriptLine): TranscriptLine[] {
  const replyMs = lines.find((other) => other.key === line.key)?.replyMs ?? line.replyMs;
  const others = lines.filter((other) => other.key !== line.key);
  const after = others.findIndex((other) => other.turn > line.turn);
  const placed = { ...line, replyMs };
  return after === -1
    ? [...others, placed]
    : [...others.slice(0, after), placed, ...others.slice(after)];
}

/** Changes the agent's line of `turn`, if it has one. */
function updateAgentTurn(
  lines: TranscriptLine[],
  turn: number,
  change: Partial<TranscriptLine>,
): TranscriptLine[] {
  const key = keyOf('agent', turn);
  return lines.map((line) => (line.key === key ? { ...line, ...change } : line));
}

/** Reserves the agent's line when it starts speaking, so the turn keeps its place. */
function reserve(lines: TranscriptLine[], turn: number): TranscriptLine[] {
  const key = keyOf('agent', turn);
  if (lines.some((line) => line.key === key)) return lines;
  const line: TranscriptLine = {
    key,
    turn,
    speaker: 'agent',
    text: null,
    interrupted: false,
    replyMs: null,
  };
  return place(lines, line);
}

/** A call that is over: nobody speaks, and a turn that never got its text is dropped. */
function over(state: TestCallState): TestCallState {
  return {
    ...state,
    agentSpeaking: false,
    hearing: null,
    lines: state.lines.filter((line) => line.text !== null),
  };
}

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
    lines: place(state.lines, {
      key: keyOf(event.speaker, event.turn_id),
      turn: event.turn_id,
      speaker: event.speaker,
      text: event.text,
      interrupted: event.interrupted,
      replyMs: null,
    }),
  }),
  'agent.audio.start': (state, event) => ({
    ...state,
    agentSpeaking: true,
    lines: reserve(state.lines, event.turn_id),
  }),
  'agent.audio.end': (state) => ({ ...state, agentSpeaking: false }),
  'agent.interrupted': (state, event) => ({
    ...state,
    agentSpeaking: false,
    lines: updateAgentTurn(state.lines, event.turn_id, { interrupted: true }),
  }),
  // Sent with the reply's first audio, so its line is already reserved.
  latency: (state, event) =>
    event.mouth_to_ear_ms === null
      ? state
      : {
          ...state,
          lines: updateAgentTurn(state.lines, event.turn_id, { replyMs: event.mouth_to_ear_ms }),
        },
  error: (state, event) => ({ ...state, serverError: event.code }),
  'session.ended': (state, event) => ({ ...over(state), phase: 'ended', endReason: event.reason }),
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
      return isOver(state.phase) ? state : { ...over(state), phase: 'ended' };
    case 'failed':
    default:
      return { ...over(state), phase: 'failed', failure: action.failure };
  }
}

/** Whether a call is under way (the page offers "End call" rather than "Start"). */
export const isInCall = (phase: CallPhase) =>
  phase === 'connecting' || phase === 'live' || phase === 'ending';
