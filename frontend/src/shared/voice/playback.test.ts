import { describe, expect, it, vi } from 'vitest';

import { PlaybackQueue, type AudioOutput } from './playback';

/** An output on a clock the test moves; records what was scheduled. */
function fakeOutput() {
  const scheduled: { at: number; length: number; ended: () => void; stop: () => void }[] = [];
  let clock = 0;
  const output: AudioOutput = {
    sampleRate: 24_000,
    now: () => clock,
    play(samples, at, onEnded) {
      const handle = { at, length: samples.length, ended: onEnded, stop: vi.fn<() => void>() };
      scheduled.push(handle);
      return handle;
    },
  };
  return {
    output,
    scheduled,
    tick: (seconds: number) => {
      clock += seconds;
    },
  };
}

const frame = () => new Float32Array(480); // 20 ms at 24 kHz

describe('PlaybackQueue', () => {
  it('drops audio before the first agent.audio.start', () => {
    const { output, scheduled } = fakeOutput();
    const queue = new PlaybackQueue(output);

    queue.push(frame());

    expect(scheduled).toHaveLength(0);
    expect(queue.busy).toBe(false);
  });

  it('schedules a turn back to back, after a little lead', () => {
    const { output, scheduled } = fakeOutput();
    const queue = new PlaybackQueue(output);

    queue.startTurn();
    queue.push(frame());
    queue.push(frame());
    queue.push(new Float32Array(0));

    expect(scheduled.map((item) => item.at)).toEqual([0.06, 0.08]);
    expect(queue.busy).toBe(true);
    scheduled[0]?.ended();
    scheduled[1]?.ended();
    expect(queue.busy).toBe(false);
  });

  it('restarts from the clock once the queue has drained', () => {
    const { output, scheduled, tick } = fakeOutput();
    const queue = new PlaybackQueue(output);

    queue.startTurn();
    queue.push(frame());
    tick(5);
    queue.push(frame());

    expect(scheduled[1]?.at).toBeCloseTo(5.06);
  });

  it('stops everything at once on barge-in and drops stragglers of that turn', () => {
    const { output, scheduled } = fakeOutput();
    const queue = new PlaybackQueue(output);
    queue.startTurn();
    queue.push(frame());
    queue.push(frame());

    queue.interrupt();
    queue.push(frame());

    expect(scheduled).toHaveLength(2);
    for (const item of scheduled) expect(item.stop).toHaveBeenCalledOnce();
    expect(queue.busy).toBe(false);

    // The next turn plays again, from the clock.
    queue.startTurn();
    queue.push(frame());
    expect(scheduled[2]?.at).toBe(0.06);
  });
});
