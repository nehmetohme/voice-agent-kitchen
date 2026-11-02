# vak-audio: frames, codecs, resampling, VAD

## Responsibility

This crate holds the audio primitives for the voice pipeline.

Today (M4a):

- **`AudioFrame`:** 20 ms (`FRAME_MS`) of mono PCM16 at its sample rate, in a fixed `[i16; MAX_FRAME_SAMPLES]` buffer (960 samples, 48 kHz), so frames move through channels without a heap allocation each. Supported rates are 8, 16, 24 and 48 kHz (320 samples at 16 kHz, 480 at 24 kHz). `new` needs exactly one frame of samples, `silent` is a frame of zeros, and `padded` pads the end of a clip with silence; each fails with `FrameError` (`UnsupportedRate`, `WrongLength`). `samples()` and `rms()` read it. `Debug` shows the rate and length, never the samples.
- **VAD:** `trait Vad: Send { fn push(&mut self, frame: &AudioFrame) -> VadEvent }`, where `VadEvent` is `Speech` or `Silence`. `EnergyVad` is the default: a frame is speech when its RMS is above both a fixed minimum (default 500, about -36 dBFS) and a multiple of a noise floor it learns from silent frames (default 3×). Turn-taking (end of turn, barge-in) is built on it in `vak-pipeline`.

Later:

- **Codecs:** G.711 μ-law/A-law (`audio-codec-algorithms`) and Opus (`opus` 0.4; it needs cmake) for telephony and WebRTC. The browser path is PCM16 only today.
- **Resampling:** `rubato` 5, using `process_into_buffer` so it doesn't allocate. Today nothing resamples: the browser sends 16 kHz and TTS returns 24 kHz, which the pipeline plays as is.
- **Model VADs:** Earshot, and Silero via `ort` behind a cargo feature, behind the same `Vad` trait.

## Dependencies

- **May depend on:** no workspace crate.
- **External crates:** today only `thiserror`. Later audio crates such as rubato, opus, earshot, `ort` (feature-gated, pinned with `=`) and `audio-codec-algorithms`. Add each to `[workspace.dependencies]` when you first use it.
- **Must not depend on:** tokio networking, sqlx, axum, reqwest, or any other `vak-*` crate.
- `vak-providers`, the adapters, `vak-pipeline` and `vak-testkit` may depend on this crate.

## Conventions

- **Hot path:** every call here runs per frame, so keep it cheap:
  - no allocation, `format!` or tracing span per frame
  - reuse buffers (`BytesMut`, fixed arrays) and write into caller-provided slices
- **No blocking or async.** Inline work must take microseconds: `EnergyVad` is one pass over the frame; later μ-law (ns), rubato on a 20 ms chunk (µs), Earshot (about 5 µs per frame). ONNX inference (Silero, turn detection) runs on dedicated threads owned by the pipeline, not here.
- **Explicit casts:** `cast_possible_truncation` warns. Write sample conversions as explicit, commented functions, such as `i16::try_from` or saturating clamps, or an `#[expect]` with its reason (as in `AudioFrame::rms`).
- **Errors:** `thiserror` (`FrameError` today; `CodecError`, `ResampleError` with the codecs). Never panic on malformed input.
- **Audio is personal data:** `Debug` never prints samples.
- **Visibility:** `pub(crate)` is the default.

## Testing

- **Unit tests** next to the code: frame lengths per rate, padding, RMS, `Debug` redaction, and `EnergyVad` on synthetic frames (loud and quiet, a noisy background raising the bar).
- **With the codecs and resampler:** `proptest` round-trips (μ-law encode→decode stays within quantisation error, resampling preserves length ratios).
- **Model VADs** run against WAV fixtures in `backend/fixtures/audio/` and assert event timings, not exact sample values.
- **Pipeline simulations** use the deterministic `vak_testkit::audio::LevelVad`, not `EnergyVad`.
- CPU cost is measured with criterion or gungraun benchmarks, not with unit tests.

## Coverage floor

No per-crate floor; this crate counts toward the global 80%.

## Example

```rust
use vak_audio::{AudioFrame, EnergyVad, Vad, VadEvent};

let mut vad = EnergyVad::default();
// Exactly one frame: 320 samples at 16 kHz.
let frame = AudioFrame::new(16_000, &samples)?;
if vad.push(&frame) == VadEvent::Speech {
    // the caller is talking
}
```
