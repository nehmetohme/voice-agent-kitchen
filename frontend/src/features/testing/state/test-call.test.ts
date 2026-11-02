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

const latency = (ms: number | null): TestCallAction =>
  event({
    type: 'latency',
    turn_id: 1,
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

  it('keeps the transcript in order, with the caller heard as they speak', () => {
    const state = run(
      ...live,
      event({
        type: 'transcript.final',
        turn_id: 0,
        speaker: 'agent',
        text: 'Hi.',
        interrupted: false,
      }),
      event({ type: 'transcript.partial', turn_id: 1, speaker: 'caller', text: 'Book a' }),
    );
    expect(state.hearing).toBe('Book a');

    const next = run(
      ...live,
      event({
        type: 'transcript.final',
        turn_id: 0,
        speaker: 'agent',
        text: 'Hi.',
        interrupted: false,
      }),
      event({ type: 'transcript.partial', turn_id: 1, speaker: 'caller', text: 'Book a' }),
      event({
        type: 'transcript.final',
        turn_id: 1,
        speaker: 'agent',
        text: 'Sure',
        interrupted: true,
      }),
      event({
        type: 'transcript.final',
        turn_id: 1,
        speaker: 'caller',
        text: 'Book a table',
        interrupted: false,
      }),
    );
    expect(next.hearing).toBeNull();
    expect(next.lines).toEqual([
      { key: 'line-0', speaker: 'agent', text: 'Hi.', interrupted: false },
      { key: 'line-1', speaker: 'agent', text: 'Sure', interrupted: true },
      { key: 'line-2', speaker: 'caller', text: 'Book a table', interrupted: false },
    ]);
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

  it('keeps the last measured reply time', () => {
    expect(run(...live, latency(640)).lastReplyMs).toBe(640);
    expect(run(...live, latency(640), latency(null)).lastReplyMs).toBe(640);
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
