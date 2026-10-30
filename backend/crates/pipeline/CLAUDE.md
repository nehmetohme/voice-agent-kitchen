# vak-pipeline: real-time voice pipeline

## Responsibility

This crate runs the real-time voice path, with one `CallSession` actor task per session:

- **Stages:** decode/resample, VAD, end-of-turn, STT, `DecisionEngine` fan-out, LLM streaming, clause chunking, TTS, pacing out.
- **Turns and plans:** turn-taking, barge-in, plan execution (the domain plan graph as a state machine) and the three-layer opt-out.
- **Budgets:** a latency budget per stage. The target is P50 ≤ 1.2 s and P95 ≤ 2.0 s mouth-to-ear.

## Dependencies

- **May depend on:** `vak-providers`, `vak-domain`, `vak-audio`. Dev-only: `vak-testkit`.
- **External crates:** `tokio`, `tokio-util` (`CancellationToken`, `TaskTracker`), `tracing`, `thiserror`, a channel crate for inference threads (`crossbeam-channel`, `flume` or `rtrb`).
- **Must not depend on:** repositories or repositories-pg, services, api, any adapter, axum, sqlx, reqwest. Transcripts and events are persisted through a sink trait that services provide.

## Conventions

- **The actor owns the turn state.** It `select!`s over inbound frames, VAD/STT events, LLM tokens, TTS audio and control (hangup, barge-in, timers). Stage I/O runs in child tasks tracked by a `TaskTracker`.
- **Bounded channels only.** Each edge declares its overflow policy in a comment next to the channel:

  | Edge | Policy |
  |---|---|
  | transport rx → VAD/STT | about 10 frames; `try_send`, drop and count `frames_dropped` |
  | TTS → pacer | await `send` (backpressure) |
  | session state | `watch` |

- **Barge-in:** use a child `CancellationToken` per turn. Frames carry a turn ID so the pacer drops stale frames in O(1).
- **No blocking in async code:** stay under about 100 µs between `.await`s.
  - ONNX inference runs on dedicated `std::thread` workers.
  - `spawn_blocking` is only for occasional work.
  - Never use `std::thread::sleep`; clippy disallows it.
- **No per-frame allocation, `format!` or tracing span.** Spans are `call` → `turn` → `stage`.
- **Pacing:** `tokio::time::interval(20 ms)` with `MissedTickBehavior::Delay`, kept 2-3 frames ahead.
- **Latency metrics:** every stage records a histogram derived from the domain `TurnTimeline` at turn end.
- **Logging:** never log transcripts or audio above `debug`.

## Testing

- **Deterministic simulation:** run under `#[tokio::test(flavor = "current_thread", start_paused = true)]`.
  - A fake transport feeds WAV fixtures from `backend/fixtures/audio/` as 20 ms frames.
  - Scripted testkit fakes emit STT/LLM/TTS events after virtual delays.
- **Assertions:** check the session event timeline (insta snapshot): barge-in cancels TTS within one frame, end-of-turn fires after the configured silence, budgets hold.
- **Inline inference in simulations,** behind its trait. Paused time doesn't wait for our OS threads.
- **CPU cost per frame:** criterion or gungraun benchmarks, not unit tests.

## Coverage floor

**90%** line coverage (`coverage.toml`).

## Example

This shows the pattern:

```rust
// transport rx -> VAD: stale audio is worthless, so drop instead of stalling the socket.
match vad_tx.try_send(frame) {
    Ok(()) => {}
    Err(TrySendError::Full(_)) => metrics.frames_dropped.add(1),
    Err(TrySendError::Closed(_)) => return Err(PipelineError::StageClosed("vad")),
}
```
