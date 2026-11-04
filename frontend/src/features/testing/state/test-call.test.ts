import { describe, expect, it } from 'vitest';

import type { ServerEvent } from '../../../shared/voice/media-protocol';
import {
  initialCall,
  isInCall,
  testCallReducer,
  type CallPhase,
  type TestCallAction,
  type TestCallState,
} from './test-call';

const started: ServerEvent = {
  type: 'session.started',
  session_id: 'abc',
  audio: {
    input: { encoding: 'pcm_s16le', sample_rate: 16_000, frame_ms: 20 },
    output: { encoding: 'pcm_s16le', sample_rate: 24_000, frame_ms: 20 },
  },
};

function run(...actions: TestCallAction[]): TestCallState {
  return actions.reduce(testCallReducer, initialCall);
}

const event = (value: ServerEvent): TestCallAction => ({ type: 'event', event: value });
const live = [{ type: 'start' }, event(started)] satisfies TestCallAction[];

const final = (
  turn: number,
  speaker: 'caller' | 'agent',
  text: string,
  interrupted = false,
): TestCallAction => event({ type: 'transcript.final', turn_id: turn, speaker, text, interrupted });

const latency = (turn: number, ms: number | null): TestCallAction =>
  event({
    type: 'latency',
    turn_id: turn,
    stt_ms: 100,
    llm_ttft_ms: 200,
    tts_first_audio_ms: 150,
    mouth_to_ear_ms: ms,
  });

describe('testCallReducer', () => {
  it('connects, then goes live on session.started', () => {
    expect(run({ type: 'start' }).phase).toBe('connecting');
    expect(run(...live).phase).toBe('live');
  });

  it('keeps the transcript, with the caller heard as they speak', () => {
    const state = run(
      ...live,
      final(0, 'agent', 'Hi.'),
      event({ type: 'transcript.partial', turn_id: 1, speaker: 'caller', text: 'Book a' }),
    );
    expect(state.hearing).toBe('Book a');

    const next = testCallReducer(state, final(1, 'caller', 'Book a table'));
    expect(next.hearing).toBeNull();
    expect(next.lines).toEqual([
      { key: 'agent-0', turn: 0, speaker: 'agent', text: 'Hi.', interrupted: false, replyMs: null },
      {
        key: 'caller-1',
        turn: 1,
        speaker: 'caller',
        text: 'Book a table',
        interrupted: false,
        replyMs: null,
      },
    ]);
  });

  it('orders lines by turn, not by arrival', () => {
    // The caller talks over the disclosure (turn 0): their final arrives before the agent's.
    const state = run(
      ...live,
      final(1, 'caller', 'Okay.'),
      final(0, 'agent', 'Hi, this is Front desk.'),
      final(3, 'caller', 'Bye.'),
      final(2, 'agent', 'Anything else?'),
    );

    expect(state.lines.map(({ turn, text }) => [turn, text])).toEqual([
      [0, 'Hi, this is Front desk.'],
      [1, 'Okay.'],
      [2, 'Anything else?'],
      [3, 'Bye.'],
    ]);
  });

  it('reserves the agent line while it speaks, then fills it with what was said', () => {
    const speaking = run(
      ...live,
      event({ type: 'agent.audio.start', turn_id: 0 }),
      final(1, 'caller', 'Okay.'),
    );
    expect(speaking.lines.map(({ key, text }) => [key, text])).toEqual([
      ['agent-0', null],
      ['caller-1', 'Okay.'],
    ]);
    // A repeated start keeps the one reserved line.
    expect(testCallReducer(speaking, event({ type: 'agent.audio.start', turn_id: 0 })).lines).toBe(
      speaking.lines,
    );

    const interrupted = run(
      ...live,
      event({ type: 'agent.audio.start', turn_id: 2 }),
      event({ type: 'agent.interrupted', turn_id: 2 }),
    );
    expect(interrupted.lines).toEqual([
      { key: 'agent-2', turn: 2, speaker: 'agent', text: null, interrupted: true, replyMs: null },
    ]);
    const said = testCallReducer(interrupted, final(2, 'agent', 'Sure', true));
    expect(said.lines).toEqual([
      { key: 'agent-2', turn: 2, speaker: 'agent', text: 'Sure', interrupted: true, replyMs: null },
    ]);
  });

  it("puts each reply's end-to-end time on its line, and keeps it when the text arrives", () => {
    const state = run(
      ...live,
      event({ type: 'agent.audio.start', turn_id: 2 }),
      latency(2, 640),
      latency(2, null),
      latency(7, 900),
      final(2, 'agent', 'Sure.'),
    );

    expect(state.lines).toEqual([
      {
        key: 'agent-2',
        turn: 2,
        speaker: 'agent',
        text: 'Sure.',
        interrupted: false,
        replyMs: 640,
      },
    ]);
  });

  it('drops a reserved line and what was being heard once the call is over', () => {
    const speaking = run(
      ...live,
      final(0, 'agent', 'Hi.'),
      event({ type: 'agent.audio.start', turn_id: 2 }),
      event({ type: 'transcript.partial', turn_id: 3, speaker: 'caller', text: 'And' }),
    );
    const kept = [
      { key: 'agent-0', turn: 0, speaker: 'agent', text: 'Hi.', interrupted: false, replyMs: null },
    ];

    const ended = testCallReducer(
      speaking,
      event({ type: 'session.ended', reason: 'opt_out', opt_out_layer: 'phrase' }),
    );
    expect(ended).toMatchObject({ lines: kept, hearing: null });
    expect(testCallReducer(speaking, { type: 'closed' })).toMatchObject({
      lines: kept,
      hearing: null,
    });
    expect(
      testCallReducer(speaking, { type: 'failed', failure: { kind: 'connection' } }).lines,
    ).toEqual(kept);
  });

  it('tracks when the agent speaks, and stops on barge-in', () => {
    const speaking = run(...live, event({ type: 'agent.audio.start', turn_id: 1 }));
    expect(speaking.agentSpeaking).toBe(true);
    expect(
      testCallReducer(speaking, event({ type: 'agent.audio.end', turn_id: 1 })).agentSpeaking,
    ).toBe(false);
    expect(
      testCallReducer(speaking, event({ type: 'agent.interrupted', turn_id: 1 })).agentSpeaking,
    ).toBe(false);
  });

  it('records server errors and ignores events it does not show', () => {
    const state = run(
      ...live,
      event({ type: 'error', code: 'provider_unavailable', message: 'The model timed out.' }),
      event({ type: 'plan.node', turn_id: 1, node_id: 'greet' }),
    );

    expect(state.serverError).toBe('provider_unavailable');
    expect(state.phase).toBe('live');
  });

  it('ends with the server reason, and the close changes nothing after', () => {
    const ended = run(
      ...live,
      event({ type: 'agent.audio.start', turn_id: 1 }),
      event({ type: 'session.ended', reason: 'opt_out', opt_out_layer: 'phrase' }),
    );
    expect(ended).toMatchObject({ phase: 'ended', endReason: 'opt_out', agentSpeaking: false });

    const closed = testCallReducer(ended, { type: 'closed' });
    expect(closed).toBe(ended);
    expect(testCallReducer(ended, event(started))).toBe(ended);
    expect(testCallReducer(ended, { type: 'hang-up' })).toBe(ended);
  });

  it('hangs up, then ends when the socket closes', () => {
    const ending = run(...live, { type: 'hang-up' });
    expect(ending.phase).toBe('ending');

    const closed = testCallReducer(ending, { type: 'closed' });
    expect(closed).toMatchObject({ phase: 'ended', endReason: null, hearing: null });
  });

  it('fails with a reason, and a new call starts clean', () => {
    const failed = run({ type: 'start' }, { type: 'failed', failure: { kind: 'microphone' } });
    expect(failed).toMatchObject({ phase: 'failed', failure: { kind: 'microphone' } });

    expect(testCallReducer(failed, { type: 'start' })).toEqual({
      ...initialCall,
      phase: 'connecting',
    });
  });

  it('remembers mute', () => {
    expect(run(...live, { type: 'mute', muted: true }).muted).toBe(true);
  });
});

describe('isInCall', () => {
  it('is true from connecting until the call is over', () => {
    const during: CallPhase[] = ['connecting', 'live', 'ending'];
    const outside: CallPhase[] = ['idle', 'ended', 'failed'];

    expect(during.every(isInCall)).toBe(true);
    expect(outside.some(isInCall)).toBe(false);
  });
});
