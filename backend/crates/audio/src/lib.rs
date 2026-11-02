//! Audio primitives for the voice pipeline: 20 ms PCM16 frames and the `Vad`
//! trait with its energy-based implementation. Codecs (Opus, G.711) and
//! resampling come with telephony.
//!
//! Code here runs on the hot path: no allocation per frame, no blocking, no
//! network. It depends on no other workspace crate.

mod frame;
mod vad;

pub use frame::{AudioFrame, FRAME_MS, FrameError, MAX_FRAME_SAMPLES};
pub use vad::{EnergyVad, Vad, VadEvent};
