// The agent's audio, scheduled back to back as it arrives, and stopped at once on barge-in.

/** Something playing that can be cut off. */
export interface Playing {
  stop(): void;
}

/** Where scheduled audio goes: Web Audio in the browser, a fake in tests. */
export interface AudioOutput {
  /** The output clock, in seconds. */
  now(): number;
  /** Plays `samples` at `at` seconds; calls `onEnded` when they finish. */
  play(samples: Float32Array, at: number, onEnded: () => void): Playing;
  readonly sampleRate: number;
}

/** A little slack before the first frame, so network jitter doesn't cause gaps. */
const LEAD_SECONDS = 0.06;

/**
 * The playback queue of `vak.media.v1`: frames of the current agent turn play in order; after
 * `agent.interrupted` everything queued stops and further frames are dropped until the next
 * `agent.audio.start`.
 */
export class PlaybackQueue {
  private readonly output: AudioOutput;
  private readonly playing = new Set<Playing>();
  private nextAt = 0;
  private accepting = false;

  constructor(output: AudioOutput) {
    this.output = output;
  }

  /** `agent.audio.start`: frames after it belong to a new turn. */
  startTurn(): void {
    this.accepting = true;
  }

  /** `agent.interrupted`: stop now, and drop stragglers of the interrupted turn. */
  interrupt(): void {
    this.accepting = false;
    this.stop();
  }

  /** A binary frame from the server. */
  push(samples: Float32Array): void {
    if (!this.accepting || samples.length === 0) return;
    const at = Math.max(this.output.now() + LEAD_SECONDS, this.nextAt);
    this.nextAt = at + samples.length / this.output.sampleRate;
    const handle = this.output.play(samples, at, () => {
      this.playing.delete(handle);
    });
    this.playing.add(handle);
  }

  /** Whether audio is still queued or playing. */
  get busy(): boolean {
    return this.playing.size > 0;
  }

  /** Stops everything at once. */
  stop(): void {
    for (const handle of this.playing) handle.stop();
    this.playing.clear();
    this.nextAt = 0;
  }
}
