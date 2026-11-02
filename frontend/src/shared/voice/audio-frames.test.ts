import { describe, expect, it } from 'vitest';

import { FrameAccumulator, floatToPcm16, INPUT_FRAME_SAMPLES, pcm16ToFloat } from './audio-frames';

function pcmBytes(...samples: number[]): ArrayBuffer {
  const bytes = new ArrayBuffer(samples.length * 2);
  const view = new DataView(bytes);
  samples.forEach((sample, i) => {
    view.setInt16(i * 2, sample, true);
  });
  return bytes;
}

describe('audio frames', () => {
  it('turns floats into clamped 16-bit samples', () => {
    expect([...floatToPcm16(new Float32Array([0, 1, -1, 0.5, 2, -3]))]).toEqual([
      0, 32_767, -32_768, 16_384, 32_767, -32_768,
    ]);
  });

  it('reads little-endian 16-bit bytes back as floats', () => {
    const samples = pcm16ToFloat(pcmBytes(0, 16_384, -32_768));

    expect([...samples]).toEqual([0, 0.5, -1]);
    // An odd trailing byte is not a sample.
    expect(pcm16ToFloat(new ArrayBuffer(3))).toHaveLength(1);
  });

  it('collects 128-sample quanta into whole 20 ms frames', () => {
    const accumulator = new FrameAccumulator();
    const quantum = new Float32Array(128).fill(0.5);

    expect(accumulator.push(quantum)).toEqual([]);
    expect(accumulator.push(quantum)).toEqual([]);
    const frames = accumulator.push(quantum);

    // 384 samples: one 320-sample frame, 64 left over for the next.
    expect(frames).toHaveLength(1);
    expect(frames[0]?.byteLength).toBe(INPUT_FRAME_SAMPLES * 2);
    expect(new DataView(frames[0] ?? new ArrayBuffer(2)).getInt16(0, true)).toBe(16_384);
    expect(accumulator.push(new Float32Array(256))).toHaveLength(1);
  });
});
