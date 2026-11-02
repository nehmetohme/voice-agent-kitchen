//! Voice activity detection: per-frame speech or silence. Turn-taking (end of turn,
//! barge-in) is built on top of it in `vak-pipeline`.

use crate::AudioFrame;

/// What a [`Vad`] heard in one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadEvent {
    Speech,
    Silence,
}

/// Classifies 20 ms frames as speech or silence. Implementations keep whatever state they
/// need between frames and must take microseconds per frame: they run inline on the
/// session's task.
pub trait Vad: Send {
    fn push(&mut self, frame: &AudioFrame) -> VadEvent;
}
