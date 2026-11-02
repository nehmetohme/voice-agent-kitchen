// PCM for the media socket. The microphone side is converted and framed in the capture
// worklet (web-audio.ts); the server's frames come back to floats here for playback.

/** Samples in one 20 ms frame of microphone audio at 16 kHz. */
export const INPUT_FRAME_SAMPLES = 320;

/** Little-endian signed 16-bit PCM bytes as float samples (-1 to 1). */
export function pcm16ToFloat(bytes: ArrayBuffer): Float32Array {
  const view = new DataView(bytes);
  const output = new Float32Array(Math.floor(bytes.byteLength / 2));
  for (let i = 0; i < output.length; i += 1) {
    output[i] = view.getInt16(i * 2, true) / 0x8000;
  }
  return output;
}
