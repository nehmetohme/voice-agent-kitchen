// A fake of the browser parts a voice session uses (WebSocket, microphone, Web Audio), installed
// as globals so the real shared/voice code runs in jsdom. Tests drive the socket by hand: open
// it, deliver server frames, and read what the page sent.
import { afterEach, beforeEach, vi } from 'vitest';

type Call = () => void;

export class FakeSocket extends EventTarget {
  static readonly instances: FakeSocket[] = [];
  binaryType: BinaryType = 'blob';
  readyState = 0;
  readonly sent: (string | ArrayBuffer)[] = [];
  readonly url: string;
  readonly protocol: string | undefined;

  constructor(url: string, protocol?: string) {
    super();
    this.url = url;
    this.protocol = protocol;
    FakeSocket.instances.push(this);
  }

  send(data: string | ArrayBuffer) {
    this.sent.push(data);
  }

  close() {
    if (this.readyState === 3) return;
    this.readyState = 3;
    this.dispatchEvent(new Event('close'));
  }

  /** The server accepted the upgrade. */
  open() {
    this.readyState = 1;
    this.dispatchEvent(new Event('open'));
  }

  /** A frame from the server: an event object (sent as JSON text) or audio bytes. */
  receive(data: object | ArrayBuffer) {
    const frame = data instanceof ArrayBuffer ? data : JSON.stringify(data);
    this.dispatchEvent(new MessageEvent('message', { data: frame }));
  }

  /** The client events sent so far, parsed. */
  events(): unknown[] {
    return this.sent.flatMap((frame) =>
      typeof frame === 'string' ? [JSON.parse(frame) as unknown] : [],
    );
  }

  audioFrames(): ArrayBuffer[] {
    return this.sent.filter((frame): frame is ArrayBuffer => frame instanceof ArrayBuffer);
  }
}

class FakeSource extends EventTarget {
  buffer: unknown = null;
  readonly connect = vi.fn<Call>();
  readonly disconnect = vi.fn<Call>();
  readonly start = vi.fn<(at: number) => void>();
  readonly stop = vi.fn<Call>();
}

export class FakeAudioContext {
  static readonly instances: FakeAudioContext[] = [];
  readonly sampleRate: number;
  readonly currentTime = 0;
  readonly destination = {};
  readonly sources: FakeSource[] = [];
  readonly audioWorklet = {
    addModule: vi.fn<(url: string) => Promise<void>>(() => Promise.resolve()),
  };
  readonly close = vi.fn<() => Promise<void>>(() => Promise.resolve());
  readonly microphoneSource = { connect: vi.fn<Call>(), disconnect: vi.fn<Call>() };

  createMediaStreamSource() {
    return this.microphoneSource;
  }

  constructor(options: { sampleRate: number }) {
    this.sampleRate = options.sampleRate;
    FakeAudioContext.instances.push(this);
  }

  createBuffer(_channels: number, length: number) {
    const data = new Float32Array(length);
    return { getChannelData: () => data };
  }

  createBufferSource() {
    const source = new FakeSource();
    this.sources.push(source);
    return source;
  }
}

class FakeWorkletNode {
  static readonly instances: FakeWorkletNode[] = [];
  readonly port = Object.assign(new EventTarget(), { start: vi.fn<Call>(), close: vi.fn<Call>() });

  constructor() {
    FakeWorkletNode.instances.push(this);
  }
}

export const microphone = {
  track: { stop: vi.fn<Call>() },
  getUserMedia: vi.fn<(constraints: MediaStreamConstraints) => Promise<unknown>>(),
  /** The worklet hands the page a quantum of microphone samples. */
  speak(samples: Float32Array) {
    for (const node of FakeWorkletNode.instances) {
      node.port.dispatchEvent(new MessageEvent('message', { data: samples }));
    }
  },
};

/** Installs the fakes before each test of the calling file and removes them after. */
export function installFakeVoiceBrowser() {
  // jsdom has no blob URLs; the capture worklet is loaded from one.
  const blobUrls = ['createObjectURL', 'revokeObjectURL'] as const;
  const saved = blobUrls.map((name) => Object.getOwnPropertyDescriptor(URL, name));
  beforeEach(() => {
    FakeSocket.instances.length = 0;
    FakeAudioContext.instances.length = 0;
    FakeWorkletNode.instances.length = 0;
    microphone.track.stop.mockClear();
    microphone.getUserMedia.mockReset();
    microphone.getUserMedia.mockResolvedValue({ getTracks: () => [microphone.track] });
    vi.stubGlobal('WebSocket', FakeSocket);
    vi.stubGlobal('AudioContext', FakeAudioContext);
    vi.stubGlobal('AudioWorkletNode', FakeWorkletNode);
    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: { getUserMedia: microphone.getUserMedia },
    });
    Object.defineProperty(URL, 'createObjectURL', {
      configurable: true,
      value: () => 'blob:vak-capture',
    });
    Object.defineProperty(URL, 'revokeObjectURL', { configurable: true, value: () => undefined });
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    blobUrls.forEach((name, index) => {
      const descriptor = saved[index];
      if (descriptor === undefined) Reflect.deleteProperty(URL, name);
      else Object.defineProperty(URL, name, descriptor);
    });
  });
}

/** The last socket the page opened. */
export function lastSocket(): FakeSocket {
  const socket = FakeSocket.instances.at(-1);
  if (socket === undefined) throw new Error('no socket was opened');
  return socket;
}
