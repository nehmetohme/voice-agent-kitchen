// The browser audio ends against the fake browser: the real capture worklet source runs
// against a fake AudioWorkletProcessor, so its conversion, framing and buffer reuse are tested.
import { describe, expect, it, vi } from 'vitest';

import {
  FakeAudioContext,
  installFakeVoiceBrowser,
  microphone,
} from '../../test/fake-voice-browser';
import { openMicrophone, openSpeaker } from './web-audio';

installFakeVoiceBrowser();

/** Opens the microphone and keeps a copy of every frame (the buffers are reused). */
async function capture() {
  const frames: Int16Array[] = [];
  const buffers: ArrayBuffer[] = [];
  const mic = await openMicrophone((frame) => {
    buffers.push(frame);
    frames.push(new Int16Array(frame.slice(0)));
  });
  return { mic, frames, buffers };
}

describe('openMicrophone', () => {
  it('converts the microphone to clamped PCM16 in 20 ms frames on the audio thread', async () => {
    const { frames } = await capture();

    const samples = new Float32Array(384);
    samples.set([0, 1, -1, 0.5, 2, -3]);
    microphone.speak(samples);

    // 384 samples: one 320-sample frame, 64 waiting for the next quantum.
    expect(frames).toHaveLength(1);
    expect(frames[0]?.length).toBe(320);
    expect(Array.from(frames[0]?.subarray(0, 6) ?? [])).toEqual([
      0, 32_767, -32_768, 16_384, 32_767, -32_768,
    ]);
    microphone.speak(new Float32Array(256));
    expect(frames).toHaveLength(2);
  });

  it('reuses the frame buffers the page hands back', async () => {
    const { buffers } = await capture();

    microphone.speak(new Float32Array(320 * 3));

    expect(buffers).toHaveLength(3);
    expect(new Set(buffers).size).toBe(1);
  });

  it('stops the tracks and closes its context', async () => {
    const { mic, frames } = await capture();
    const context = FakeAudioContext.instances.find((each) => each.sampleRate === 16_000);

    mic.close();
    microphone.speak(new Float32Array(320));

    expect(microphone.track.stop).toHaveBeenCalledOnce();
    expect(context?.microphoneSource.disconnect).toHaveBeenCalledOnce();
    expect(context?.close).toHaveBeenCalledOnce();
    expect(frames).toHaveLength(0);
  });
});

describe('openSpeaker', () => {
  it('plays 24 kHz buffers at their time and stops them on request', () => {
    const speaker = openSpeaker();
    const context = FakeAudioContext.instances.at(-1);
    const ended = vi.fn<() => void>();

    const playing = speaker.play(new Float32Array(480), 0.5, ended);

    expect(speaker.sampleRate).toBe(24_000);
    expect(speaker.now()).toBe(0);
    const source = context?.sources[0];
    expect(source?.start).toHaveBeenCalledWith(0.5);
    source?.dispatchEvent(new Event('ended'));
    expect(ended).toHaveBeenCalledOnce();
    playing.stop();
    expect(source?.stop).toHaveBeenCalledOnce();
    speaker.close();
    expect(context?.close).toHaveBeenCalledOnce();
  });
});
