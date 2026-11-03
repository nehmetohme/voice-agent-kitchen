// The browser's audio ends of a voice session: the microphone through an AudioWorklet at
// 16 kHz, and the speaker as a 24 kHz context the playback queue schedules into.
import { INPUT_FRAME_SAMPLES } from './audio-frames';
import { INPUT_SAMPLE_RATE, OUTPUT_SAMPLE_RATE } from './media-protocol';
import type { AudioOutput } from './playback';

/**
 * The capture worklet, on the audio thread: converts each 128-sample render quantum to
 * little-endian PCM16 straight into a 20 ms frame, and posts each full frame (640 bytes) as a
 * transferable buffer. The page hands every buffer back once sent, so in steady state nothing
 * is allocated and the main thread converts nothing: 50 messages a second, not 375.
 */
const CAPTURE_WORKLET = `
const FRAME_BYTES = ${INPUT_FRAME_SAMPLES * 2};
class VakCapture extends AudioWorkletProcessor {
  constructor() {
    super();
    this.free = [];
    this.port.onmessage = (event) => { this.free.push(event.data); };
    this.next();
  }
  next() {
    this.frame = new DataView(this.free.pop() || new ArrayBuffer(FRAME_BYTES));
    this.offset = 0;
  }
  process(inputs) {
    const channel = inputs[0] && inputs[0][0];
    if (!channel) return true;
    for (let i = 0; i < channel.length; i += 1) {
      const sample = Math.max(-1, Math.min(1, channel[i]));
      this.frame.setInt16(this.offset, Math.round(sample < 0 ? sample * 0x8000 : sample * 0x7fff), true);
      this.offset += 2;
      if (this.offset === FRAME_BYTES) {
        const full = this.frame.buffer;
        this.port.postMessage(full, [full]);
        this.next();
      }
    }
    return true;
  }
}
registerProcessor('vak-capture', VakCapture);
`;

export interface Microphone {
  close(): void;
}

/**
 * Opens the microphone (echo cancellation on) and hands over each 20 ms frame of 16 kHz
 * PCM16. A frame is only valid during the call: it goes back to the worklet afterwards.
 */
export async function openMicrophone(onFrame: (frame: ArrayBuffer) => void): Promise<Microphone> {
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
  const onMessage = ({ data }: MessageEvent<ArrayBuffer>) => {
    onFrame(data);
    node.port.postMessage(data, [data]);
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
