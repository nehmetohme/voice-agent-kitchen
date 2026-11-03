//! A fake browser for the media transport: it plays a caller's microphone into a session
//! and records what the server sends.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tokio::time::Instant;
use vak_audio::AudioFrame;
use vak_providers::media::{
    INPUT_FRAME_SAMPLES, INPUT_SAMPLE_RATE, InputFramer, MediaTransport, Outbound, PeerSender,
    media_channel,
};

use crate::providers::lock;

const FRAME: Duration = Duration::from_millis(20);
/// How long `silence_until` waits before it gives up.
const PATIENCE: Duration = Duration::from_secs(120);

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
    sender: PeerSender,
    framer: Mutex<InputFramer>,
    log: Arc<Mutex<Vec<Sent>>>,
    closed: Arc<AtomicBool>,
}

/// A session's transport, and the fake client on its other end. Call it inside a tokio
/// runtime: it spawns the recorder.
#[must_use]
pub fn fake_transport() -> (MediaTransport, FakeClient) {
    let (transport, peer) = media_channel();
    let (sender, mut outbound) = peer.into_parts();
    let log = Arc::new(Mutex::new(Vec::new()));
    let closed = Arc::new(AtomicBool::new(false));
    let (recorded, done) = (Arc::clone(&log), Arc::clone(&closed));
    tokio::spawn(async move {
        while let Some(item) = outbound.recv().await {
            let at = Instant::now();
            let sent = match item {
                Outbound::Event(event) => Sent::Event {
                    at,
                    event: serde_json::from_str(&event.to_json()).unwrap_or(Value::Null),
                },
                Outbound::Audio(frame) => Sent::Audio {
                    at,
                    samples: frame.samples().len(),
                    sample_rate: frame.sample_rate(),
                },
                _ => continue,
            };
            lock(&recorded).push(sent);
        }
        done.store(true, Ordering::SeqCst);
    });
    (
        transport,
        FakeClient {
            sender,
            framer: Mutex::new(InputFramer::default()),
            log,
            closed,
        },
    )
}

impl FakeClient {
    /// Sends `event` as a text frame. JSON that is not a client event reaches the session
    /// as an invalid message.
    pub async fn send(&self, event: Value) {
        // A session that is already gone has nobody to tell.
        let _ = self.sender.push_text(&event.to_string()).await;
    }

    /// Sends a binary frame as raw bytes, cut into 20 ms frames the way the WebSocket bridge
    /// cuts them: an odd length reaches the session as unsupported audio.
    pub fn send_bytes(&self, bytes: &[u8]) {
        // Dropped or refused audio is what a real socket would lose too.
        let _ = lock(&self.framer).push(&self.sender, bytes);
    }

    /// Speaks for `duration`.
    pub async fn speak(&self, duration: Duration) {
        self.frames(duration, &speech()).await;
    }

    /// Stays silent for `duration`, with the microphone open.
    pub async fn silence(&self, duration: Duration) {
        self.frames(duration, &[0; INPUT_FRAME_SAMPLES]).await;
    }

    /// Stays silent until `done` holds for everything sent so far.
    ///
    /// # Panics
    /// When `done` still fails after 120 s of tokio time, or the server closes the session
    /// first; the message names `what` was awaited and lists the events received.
    pub async fn silence_until(&self, what: &str, done: impl Fn(&[Sent]) -> bool) {
        let started = Instant::now();
        loop {
            let log = self.log();
            if done(&log) {
                return;
            }
            assert!(
                !self.closed.load(Ordering::SeqCst),
                "the session closed before {what}; it sent {}",
                summary(&log)
            );
            assert!(
                started.elapsed() < PATIENCE,
                "no {what} after {PATIENCE:?}; the session sent {}",
                summary(&log)
            );
            let _ = self
                .sender
                .push_audio(microphone(&[0; INPUT_FRAME_SAMPLES]));
            tokio::time::sleep(FRAME).await;
        }
    }

    /// Everything the server sent so far, in arrival order.
    #[must_use]
    pub fn log(&self) -> Vec<Sent> {
        lock(&self.log).clone()
    }

    async fn frames(&self, duration: Duration, frame: &[i16]) {
        let count = duration.as_millis() / FRAME.as_millis();
        for _ in 0..count {
            // A dropped or refused frame is what a real microphone would lose too.
            let _ = self.sender.push_audio(microphone(frame));
            tokio::time::sleep(FRAME).await;
        }
    }
}

/// A 16 kHz microphone frame of `samples`, which hold exactly one frame.
fn microphone(samples: &[i16]) -> AudioFrame {
    AudioFrame::padded(INPUT_SAMPLE_RATE, samples)
        .unwrap_or_else(|_| unreachable!("16 kHz is supported"))
}

/// One frame of speech: a loud 500 Hz square wave.
fn speech() -> [i16; INPUT_FRAME_SAMPLES] {
    std::array::from_fn(|i| if (i / 16) % 2 == 0 { 8_000 } else { -8_000 })
}

/// The event types received, with turn IDs, and how many audio frames: for failure messages.
fn summary(log: &[Sent]) -> String {
    let mut parts = Vec::new();
    let mut frames = 0;
    for sent in log {
        match sent {
            Sent::Audio { .. } => frames += 1,
            Sent::Event { event, .. } => {
                if frames > 0 {
                    parts.push(format!("{frames} frame(s)"));
                    frames = 0;
                }
                let kind = event["type"].as_str().unwrap_or("?");
                parts.push(
                    event["turn_id"]
                        .as_u64()
                        .map_or_else(|| kind.to_owned(), |turn| format!("{kind}#{turn}")),
                );
            }
        }
    }
    if frames > 0 {
        parts.push(format!("{frames} frame(s)"));
    }
    if parts.is_empty() {
        "nothing".to_owned()
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vak_providers::media::{ClientEvent, Inbound, ServerEvent};

    use super::*;

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn the_client_paces_frames_and_records_what_arrives() {
        let (transport, client) = fake_transport();
        let (mut inbound, outbound) = transport.into_parts();

        client.send(json!({"type": "session.end"})).await;
        assert_eq!(
            inbound.recv().await,
            Some(Inbound::Event(ClientEvent::SessionEnd))
        );
        let started = Instant::now();
        client.speak(Duration::from_millis(60)).await;
        client.silence(Duration::from_millis(40)).await;
        assert_eq!(started.elapsed(), Duration::from_millis(100));
        let mut frames = Vec::new();
        while let Ok(Inbound::Audio(frame)) = inbound.try_recv() {
            frames.push(
                frame
                    .samples()
                    .iter()
                    .map(|s| s.unsigned_abs())
                    .max()
                    .unwrap_or(0),
            );
        }
        assert_eq!(frames, [8_000, 8_000, 8_000, 0, 0]);

        outbound
            .send(Outbound::Event(ServerEvent::AgentAudioStart { turn_id: 4 }))
            .await
            .unwrap();
        outbound
            .send(Outbound::Audio(
                vak_audio::AudioFrame::silent(24_000).unwrap(),
            ))
            .await
            .unwrap();
        client
            .silence_until("the frame", |log| log.len() == 2)
            .await;
        let log = client.log();
        assert!(
            matches!(&log[0], Sent::Event { event, .. } if event == &json!({"type": "agent.audio.start", "turn_id": 4}))
        );
        assert!(matches!(
            log[1],
            Sent::Audio {
                samples: 480,
                sample_rate: 24_000,
                ..
            }
        ));
        assert_eq!(summary(&log), "agent.audio.start#4, 1 frame(s)");
        assert_eq!(summary(&[]), "nothing");
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    #[should_panic(expected = "the session closed before the end")]
    async fn waiting_on_a_closed_session_fails() {
        let (transport, client) = fake_transport();
        drop(transport);
        client.silence_until("the end", |_| false).await;
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    #[should_panic(expected = "no reply after")]
    async fn waiting_forever_fails() {
        let (_transport, client) = fake_transport();
        client.silence_until("reply", |_| false).await;
    }
}
