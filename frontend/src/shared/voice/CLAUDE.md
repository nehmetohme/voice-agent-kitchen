# src/shared/voice/

The browser side of a voice session over the media WebSocket (`vak.media.v1`,
`contracts/media/`). Nothing here renders UI.

## Shape

| File                | What                                                                                                                                                                                                                                                                                                                                                        |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `media-protocol.ts` | Zod schemas of the server events, `parseServerEvent` (`null` for anything invalid or unknown), `encodeClientEvent`, rates (16 kHz in, 24 kHz out, 20 ms frames). Pinned by `media-protocol.contract.test.ts`                                                                                                                                                |
| `audio-frames.ts`   | `pcm16ToFloat` for the server's frames, and `INPUT_FRAME_SAMPLES` (320)                                                                                                                                                                                                                                                                                     |
| `playback.ts`       | `PlaybackQueue`: schedules a turn's frames back to back; `interrupt()` stops everything at once and drops stragglers until the next `agent.audio.start`                                                                                                                                                                                                     |
| `web-audio.ts`      | `openMicrophone(onFrame)`: getUserMedia with echo cancellation and a 16 kHz context; the capture AudioWorklet (loaded from a Blob) converts to PCM16 and cuts 20 ms frames on the audio thread, posting each 640-byte frame as a transferable that the page hands back after sending, so steady state allocates nothing. `openSpeaker` is the 24 kHz output |
| `voice-session.ts`  | `openVoiceSession(mediaPath, handlers, deps?)`: speaker, microphone and socket; sends `session.start` on open and audio only after it; `setMuted`; `end()` sends `session.end` and closes after 2 s if the server doesn't                                                                                                                                   |

`VoiceDeps` (socket, microphone, speaker, page URL) is injectable; the browser versions are the
default.

## May import / be imported by

- Imports: browser APIs, `zod` and `shared/lib` only.
- Imported by: `features/testing/state/` (the test call hook). Views never touch
  `getUserMedia`, `AudioContext` or sockets.

## Conventions

- **Barge-in:** `agent.interrupted` stops playback at once (every scheduled source stopped) and
  drops the interrupted turn's late frames.
- **Strict input:** a server frame that fails the schema is ignored, never thrown.
- **No audio or transcript text in logs** (PII).
- **Clean up on close:** tracks stopped, worklet port closed, both audio contexts closed.
- Open a session from a click: browsers only let an audio context play after a user gesture.

## Testing

- `media-protocol.contract.test.ts` checks the schemas against `contracts/media/` (contract,
  `voice-core` v1).
- `voice-session.test.ts` runs the real session, capture and playback code against
  `src/test/fake-voice-browser.ts` (fake WebSocket, getUserMedia, AudioContext, worklet).
- `web-audio.test.ts` runs the real worklet source against a fake `AudioWorkletProcessor`:
  conversion and clamping, framing, buffer reuse, and the speaker.
- `audio-frames.test.ts` and `playback.test.ts` cover the pure parts.
- A conversation with fake media in Playwright comes with the `test-call` golden path (M5).

## Example

```ts
const session = await openVoiceSession(mediaUrl, {
  onEvent: (event) => dispatch({ type: 'event', event }),
  onClosed: () => dispatch({ type: 'closed' }),
});
session.setMuted(true);
session.end();
```
