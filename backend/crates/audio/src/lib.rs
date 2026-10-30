//! Audio primitives for the voice pipeline: 20 ms PCM16 frames, codecs
//! (PCM, Opus, G.711 mu-law), resampling and the `Vad` trait with its
//! implementations.
//!
//! Code here runs on the hot path: no allocation per frame, no blocking, no
//! network. It depends on no other workspace crate.
