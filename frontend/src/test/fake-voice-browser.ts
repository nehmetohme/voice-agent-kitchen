// A fake of the browser parts a voice session uses (WebSocket, microphone, Web Audio), installed
// as globals so the real shared/voice code runs in jsdom, the capture worklet included: its
// source is read back from the blob URL and run against a fake AudioWorkletProcessor. Tests
// drive the socket by hand: open it, deliver server frames, and read what the page sent.
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

  /** Copies binary data, as a real socket does: the caller may reuse its buffer. */
  send(data: string | ArrayBuffer) {
    this.sent.push(typeof data === 'string' ? data : data.slice(0));
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

/** One end of a message channel; delivery to the other end is immediate. */
class FakePort extends EventTarget {
  peer: FakePort | undefined;
  onmessage: ((event: MessageEvent) => void) | null = null;
  readonly start = vi.fn<Call>();
  readonly close = vi.fn<Call>();

  postMessage(data: unknown) {
    const event = new MessageEvent('message', { data });
    this.peer?.onmessage?.(event);
    this.peer?.dispatchEvent(event);
  }
}

function portPair(): [FakePort, FakePort] {
  const page = new FakePort();
  const worklet = new FakePort();
  page.peer = worklet;
  worklet.peer = page;
  return [page, worklet];
}

interface Processor {
  process(inputs: Float32Array[][]): boolean;
}
type ProcessorClass = new () => Processor;

const processors = new Map<string, ProcessorClass>();
let workletPort: FakePort | undefined;

/** The base class the worklet's processor extends: it gets the worklet end of the port. */
class FakeAudioWorkletProcessor {
  readonly port: FakePort;

  constructor() {
    if (workletPort === undefined) throw new Error('a processor is made by its node');
    this.port = workletPort;
  }
}

function registerProcessor(name: string, processor: ProcessorClass) {
  processors.set(name, processor);
}

/** Runs a worklet module's source, as `audioWorklet.addModule` would. */
function loadWorklet(source: string) {
  // Test only: the module is our own CAPTURE_WORKLET string.
  // oxlint-disable-next-line typescript/no-implied-eval
  const module = new Function('AudioWorkletProcessor', 'registerProcessor', source);
  Reflect.apply(module, undefined, [FakeAudioWorkletProcessor, registerProcessor]);
}

const blobs = new Map<string, Blob>();

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
    addModule: vi.fn<(url: string) => Promise<void>>(async (url) => {
      const blob = blobs.get(url);
      if (blob === undefined) throw new Error(`no module at ${url}`);
      loadWorklet(await blob.text());
    }),
  };
  readonly close = vi.fn<() => Promise<void>>(() => Promise.resolve());
  readonly microphoneSource = { connect: vi.fn<Call>(), disconnect: vi.fn<Call>() };

  constructor(options: { sampleRate: number }) {
    this.sampleRate = options.sampleRate;
    FakeAudioContext.instances.push(this);
  }

  createMediaStreamSource() {
    return this.microphoneSource;
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
  readonly port: FakePort;
  readonly processor: Processor;

  constructor(_context: unknown, name: string) {
    const Processor = processors.get(name);
    if (Processor === undefined) throw new Error(`no processor named ${name}`);
    const [page, worklet] = portPair();
    workletPort = worklet;
    this.processor = new Processor();
    workletPort = undefined;
    this.port = page;
    FakeWorkletNode.instances.push(this);
  }
}

export const microphone = {
  track: { stop: vi.fn<Call>() },
  getUserMedia: vi.fn<(constraints: MediaStreamConstraints) => Promise<unknown>>(),
  /** The audio thread renders `samples` through every capture worklet, 128 at a time. */
  speak(samples: Float32Array) {
    for (let at = 0; at < samples.length; at += 128) {
      const quantum = samples.subarray(at, at + 128);
      for (const node of FakeWorkletNode.instances) node.processor.process([[quantum]]);
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
    blobs.clear();
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
      value: (blob: Blob) => {
        const url = `blob:vak-${blobs.size}`;
        blobs.set(url, blob);
        return url;
      },
    });
    Object.defineProperty(URL, 'revokeObjectURL', {
      configurable: true,
      value: (url: string) => blobs.delete(url),
    });
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
