// The browser's audio ends of a voice session: the microphone through an AudioWorklet at
// 16 kHz, and the speaker as a 24 kHz context the playback queue schedules into.
// Thin on purpose: everything worth testing lives in audio-frames.ts and playback.ts.
import { INPUT_SAMPLE_RATE, OUTPUT_SAMPLE_RATE } from './media-protocol';
import type { AudioOutput } from './playback';

/** Hands every 128-sample render quantum of the microphone to the page. */
const CAPTURE_WORKLET = `
class VakCapture extends AudioWorkletProcessor {
  process(inputs) {
    const channel = inputs[0] && inputs[0][0];
    if (channel) this.port.postMessage(channel.slice(0));
    return true;
  }
}
registerProcessor('vak-capture', VakCapture);
`;

export interface Microphone {
  close(): void;
}

/** Opens the microphone (echo cancellation on) and streams its samples at 16 kHz. */
export async function openMicrophone(
  onSamples: (samples: Float32Array) => void,
): Promise<Microphone> {
  const stream = await navigator.mediaDevices.getUserMedia({
    audio: {
      channelCount: 1,
      echoCancellation: true,
      noiseSuppression: true,
      autoGainControl: true,
    },
  });
  const context = new AudioContext({ sampleRate: INPUT_SAMPLE_RATE });
  const url = URL.createObjectURL(new Blob([CAPTURE_WORKLET], { type: 'text/javascript' }));
  try {
    await context.audioWorklet.addModule(url);
  } finally {
    URL.revokeObjectURL(url);
  }
  const source = context.createMediaStreamSource(stream);
  const node = new AudioWorkletNode(context, 'vak-capture', { numberOfOutputs: 0 });
  const onMessage = (event: MessageEvent<Float32Array>) => {
    onSamples(event.data);
  };
  node.port.addEventListener('message', onMessage);
  node.port.start();
  source.connect(node);
  return {
    close() {
      for (const track of stream.getTracks()) track.stop();
      source.disconnect();
      node.port.removeEventListener('message', onMessage);
      node.port.close();
      void context.close();
    },
  };
}

export interface Speaker extends AudioOutput {
  close(): void;
}

/** A 24 kHz output for the agent's audio. Create it from a click, so it may play. */
export function openSpeaker(): Speaker {
  const context = new AudioContext({ sampleRate: OUTPUT_SAMPLE_RATE });
  return {
    sampleRate: OUTPUT_SAMPLE_RATE,
    now: () => context.currentTime,
    play(samples, at, onEnded) {
      const buffer = context.createBuffer(1, samples.length, OUTPUT_SAMPLE_RATE);
      buffer.getChannelData(0).set(samples);
      const source = context.createBufferSource();
      source.buffer = buffer;
      source.connect(context.destination);
      source.addEventListener('ended', onEnded, { once: true });
      source.start(at);
      return {
        stop() {
          source.removeEventListener('ended', onEnded);
          source.stop();
          source.disconnect();
        },
      };
    },
    close() {
      void context.close();
    },
  };
}
