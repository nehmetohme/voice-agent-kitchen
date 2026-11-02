import { describe, expect, it } from 'vitest';

import { pcm16ToFloat } from './audio-frames';

function pcmBytes(...samples: number[]): ArrayBuffer {
  const bytes = new ArrayBuffer(samples.length * 2);
  const view = new DataView(bytes);
  samples.forEach((sample, i) => {
    view.setInt16(i * 2, sample, true);
  });
  return bytes;
}

describe('audio frames', () => {
  it('reads little-endian 16-bit bytes back as floats', () => {
    const samples = pcm16ToFloat(pcmBytes(0, 16_384, -32_768));

    expect([...samples]).toEqual([0, 0.5, -1]);
    // An odd trailing byte is not a sample.
    expect(pcm16ToFloat(new ArrayBuffer(3))).toHaveLength(1);
  });
});
