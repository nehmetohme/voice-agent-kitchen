# src/shared/voice/

The browser side of a voice session, behind a port, like the backend's providers. Empty until
milestone M4 (voice core). Nothing here renders UI.

## Planned shape

```
voice/
  voice-session.ts      the port: VoiceSession interface and event types
  adapters/
    websocket-pcm.ts    AudioWorklet capture at 16 kHz + playback queue over vak.media.v1
    webrtc.ts           only if the transport spike picks WebRTC
    fake.ts             scripted events for unit tests
  worklets/             AudioWorklet processors (plain JS modules loaded by URL)
```

```ts
export interface VoiceSession {
  connect(agentVersionId: string): Promise<void>;
  mute(muted: boolean): void;
  end(): Promise<void>;
  on<E extends VoiceEvent['type']>(
    type: E,
    listener: (event: Extract<VoiceEvent, { type: E }>) => void,
  ): () => void;
}
// events: transcript.partial | transcript.final | agent.state | metrics.latency | error
```

## May import / be imported by

- Imports: browser APIs and `shared/lib` only. Event shapes come from the media protocol in
  `contracts/media/` (generated or mirrored types, checked in tests).
- Imported by: `features/testing/state/` (the XState voice-session machine). Views never touch
  `getUserMedia`, `AudioContext` or sockets.

## Conventions

- The port is transport-neutral; swapping WebSocket for WebRTC changes only an adapter.
- Every adapter emits the same events in the same order for the same input; the fake proves
  the contract.
- Barge-in: stop playback within one audio frame of `agent.state → interrupted`.
- No audio or transcript text in logs (PII). Latency metrics only.
- Clean up on `end()`: tracks stopped, worklets disconnected, sockets closed.

## Testing

- Each adapter passes a shared port test suite using the fake microphone.
- E2E uses Chromium's fake media flags with `e2e/fixtures/audio/*.wav`.
- Coverage counts toward the 80 % frontend floor; the voice-session machine in
  `features/testing/state/` is held to 90 %.

## Example

```ts
const session = createFakeVoiceSession([{ type: 'transcript.final', text: 'Hello' }]);
const stop = session.on('transcript.final', (event) => transcript.push(event.text));
await session.connect('ver_1');
```
