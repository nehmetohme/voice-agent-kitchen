//! A fake browser for the media transport: it plays a caller's microphone into a session
//! and records what the server sends.

use std::time::Duration;

use serde_json::Value;
use tokio::time::Instant;
use vak_providers::media::{MediaPeer, MediaTransport, media_channel};

/// Something the server sent, stamped with the (virtual) time it arrived.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Sent {
    /// A text frame: the event as JSON, exactly as it goes on the wire.
    Event { at: Instant, event: Value },
    /// A binary frame of agent audio.
    Audio {
        at: Instant,
        samples: usize,
        sample_rate: u32,
    },
}

/// The test's end of a session: a caller with a microphone that is always on.
///
/// `speak`, `silence` and `silence_until` send one 20 ms frame of 16 kHz audio every 20 ms of
/// tokio time, as a browser does: speech is a loud square wave, silence is zeros. A
/// recorder task stores every server message the moment it arrives.
#[derive(Debug)]
pub struct FakeClient {
    // CONTRACT-STUB
    _peer: MediaPeer,
}

/// A session's transport, and the fake client on its other end. Call it inside a tokio
/// runtime: it spawns the recorder.
#[must_use]
pub fn fake_transport() -> (MediaTransport, FakeClient) {
    // CONTRACT-STUB
    let (transport, peer) = media_channel();
    (transport, FakeClient { _peer: peer })
}

// CONTRACT-STUB: the methods await nothing yet.
#[expect(clippy::unused_async, reason = "contract stub")]
impl FakeClient {
    /// Sends `event` as a text frame. JSON that is not a client event reaches the session
    /// as an invalid message.
    pub async fn send(&self, event: Value) {
        drop(event);
    }

    /// Speaks for `duration`.
    pub async fn speak(&self, duration: Duration) {
        let _ = duration;
    }

    /// Stays silent for `duration`, with the microphone open.
    pub async fn silence(&self, duration: Duration) {
        let _ = duration;
    }

    /// Stays silent until `done` holds for everything sent so far.
    ///
    /// # Panics
    /// When `done` still fails after 120 s of tokio time, or the server closes the session
    /// first; the message names `what` was awaited and lists the events received.
    pub async fn silence_until(&self, what: &str, done: impl Fn(&[Sent]) -> bool) {
        let _ = (what, done(&[]));
    }

    /// Everything the server sent so far, in arrival order.
    #[must_use]
    pub fn log(&self) -> Vec<Sent> {
        // CONTRACT-STUB
        Vec::new()
    }
}
