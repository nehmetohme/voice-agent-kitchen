// PCM conversions for the media socket: microphone floats to 16-bit frames, server frames back
// to floats. Plain functions over typed arrays, so they run the same in tests and the browser.

/** Samples in one 20 ms frame of microphone audio at 16 kHz. */
export const INPUT_FRAME_SAMPLES = 320;

/** Float samples (-1 to 1) as signed 16-bit PCM, clamped. */
export function floatToPcm16(input: Float32Array): Int16Array {
  const output = new Int16Array(input.length);
  for (let i = 0; i < input.length; i += 1) {
    const sample = Math.max(-1, Math.min(1, input[i] ?? 0));
    output[i] = sample < 0 ? Math.round(sample * 0x8000) : Math.round(sample * 0x7fff);
  }
  return output;
}

/** Little-endian signed 16-bit PCM bytes as float samples (-1 to 1). */
export function pcm16ToFloat(bytes: ArrayBuffer): Float32Array {
  const view = new DataView(bytes);
  const output = new Float32Array(Math.floor(bytes.byteLength / 2));
  for (let i = 0; i < output.length; i += 1) {
    output[i] = view.getInt16(i * 2, true) / 0x8000;
  }
  return output;
}

/**
 * Collects float samples into whole 20 ms frames of little-endian PCM16 bytes: the audio
 * worklet hands over 128 samples at a time, the socket wants 320.
 */
export class FrameAccumulator {
  private readonly frame = new Int16Array(INPUT_FRAME_SAMPLES);
  private filled = 0;

  /** Adds samples; returns every frame they completed, as bytes ready to send. */
  push(samples: Float32Array): ArrayBuffer[] {
    const frames: ArrayBuffer[] = [];
    const pcm = floatToPcm16(samples);
    for (const sample of pcm) {
      this.frame[this.filled] = sample;
      this.filled += 1;
      if (this.filled === INPUT_FRAME_SAMPLES) {
        frames.push(littleEndian(this.frame));
        this.filled = 0;
      }
    }
    return frames;
  }
}

function littleEndian(samples: Int16Array): ArrayBuffer {
  const bytes = new ArrayBuffer(samples.length * 2);
  const view = new DataView(bytes);
  samples.forEach((sample, i) => {
    view.setInt16(i * 2, sample, true);
  });
  return bytes;
}
