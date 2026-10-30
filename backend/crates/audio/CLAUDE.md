# vak-audio: frames, codecs, resampling, VAD

## Responsibility

This crate holds the audio primitives for the voice pipeline:

- **`AudioFrame`:** a 20 ms mono PCM16 frame (320 samples at 16 kHz), with its sample rate.
- **Codecs:** PCM16, G.711 μ-law/A-law (`audio-codec-algorithms`) and Opus (`opus` 0.4; it needs cmake).
- **Resampling:** `rubato` 5, using `process_into_buffer` so it doesn't allocate.
- **VAD:** `trait Vad { fn push(&mut self, frame: &AudioFrame) -> VadEvent }`, with Earshot as the default and Silero via `ort` behind a cargo feature.

## Dependencies

- **May depend on:** no workspace crate.
- **External crates:** audio crates such as rubato, opus, earshot, `ort` (feature-gated, pinned with `=`), `audio-codec-algorithms` and `thiserror`. Add each to `[workspace.dependencies]` when you first use it.
- **Must not depend on:** tokio networking, sqlx, axum, reqwest, or any other `vak-*` crate.
- `vak-providers`, the adapters and `vak-pipeline` may depend on this crate.

## Conventions

- **Hot path:** every call here runs per frame, so keep it cheap:
  - no allocation, `format!` or tracing span per frame
  - reuse buffers (`BytesMut`, fixed arrays) and write into caller-provided slices
- **No blocking or async.** Inline work must take microseconds: μ-law (ns), rubato on a 20 ms chunk (µs), Earshot (about 5 µs per frame). ONNX inference (Silero, turn detection) runs on dedicated threads owned by the pipeline, not here.
- **Explicit casts:** `cast_possible_truncation` warns. Write sample conversions as explicit, commented functions, such as `i16::try_from` or saturating clamps.
- **Errors:** `thiserror` (`CodecError`, `ResampleError`). Never panic on malformed input.
- **Visibility:** `pub(crate)` is the default.

## Testing

- Unit tests per codec and resampler.
- `proptest` round-trips: μ-law encode→decode stays within quantisation error, and resampling preserves length ratios.
- VAD implementations run against WAV fixtures in `backend/fixtures/audio/` (read with `hound`). They assert event timings, not exact sample values.
- CPU cost is measured with criterion or gungraun benchmarks, not with unit tests.

## Coverage floor

No per-crate floor; this crate counts toward the global 80%.

## Example

This shows the pattern:

```rust
/// Decodes G.711 μ-law bytes into `out`. Never allocates.
pub fn decode_ulaw(input: &[u8], out: &mut [i16]) -> Result<usize, CodecError> {
    if out.len() < input.len() {
        return Err(CodecError::BufferTooSmall { needed: input.len(), got: out.len() });
    }
    for (dst, &byte) in out.iter_mut().zip(input) {
        *dst = ULAW_TO_PCM[usize::from(byte)];
    }
    Ok(input.len())
}
```
