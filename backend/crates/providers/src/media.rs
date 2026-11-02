//! The media transport port and the `vak.media.v1` events (`contracts/media/`).
//!
//! A session's transport is a pair of bounded channels: [`MediaTransport`] is the pipeline's
//! end, [`MediaPeer`] the other end, held by the browser WebSocket bridge (`vak-api`) or a
//! test's fake client (`vak-testkit`). Text frames are [`ClientEvent`] and [`ServerEvent`]
//! as JSON; binary frames are PCM16 audio, 16 kHz in and 24 kHz out, in 20 ms frames.
//!
//! | Edge | Capacity | When full |
//! |---|---|---|
//! | peer → pipeline, audio | [`INBOUND_CAPACITY`] items | dropped: late audio is worthless, and the socket reader never stalls |
//! | peer → pipeline, events | shared | the peer waits (events are rare) |
//! | pipeline → peer | [`OUTBOUND_CAPACITY`] items | the pipeline waits: the bridge only writes to the socket |

use tokio::sync::mpsc;
use vak_audio::AudioFrame;
use vak_domain::{SessionEndReason, Speaker as DomainSpeaker};

use crate::SensitiveText;

/// The WebSocket subprotocol.
pub const PROTOCOL: &str = "vak.media.v1";
/// Sample rate of the client's audio (PCM s16le mono).
pub const INPUT_SAMPLE_RATE: u32 = 16_000;
/// Sample rate of the server's audio (PCM s16le mono): Kokoro's native rate.
pub const OUTPUT_SAMPLE_RATE: u32 = 24_000;
/// Length of an audio frame in either direction.
pub const FRAME_MS: u32 = vak_audio::FRAME_MS;
/// Items the peer can queue for the pipeline: about 1.3 s of 20 ms frames.
pub const INBOUND_CAPACITY: usize = 64;
/// Items the pipeline can queue for the peer.
pub const OUTBOUND_CAPACITY: usize = 256;

/// The `type` of every event a client may send.
pub const CLIENT_EVENT_TYPES: &[&str] = &["session.start", "client.mute", "session.end"];
/// The `type` of every event the server may send.
pub const SERVER_EVENT_TYPES: &[&str] = &[
    "session.started",
    "transcript.partial",
    "transcript.final",
    "agent.audio.start",
    "agent.audio.end",
    "agent.interrupted",
    "plan.node",
    "decision",
    "latency",
    "error",
    "session.ended",
];

/// A text frame from the client.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum ClientEvent {
    /// `session.start`: the client is ready to talk.
    #[serde(rename = "session.start")]
    SessionStart {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        audio: Option<InputAudio>,
    },
    /// `client.mute`: the microphone is muted or unmuted.
    #[serde(rename = "client.mute")]
    Mute { muted: bool },
    /// `session.end`: the caller hung up.
    #[serde(rename = "session.end")]
    SessionEnd,
}

impl ClientEvent {
    /// Parses a text frame; `None` when it is not a client event.
    #[must_use]
    pub fn from_json(text: &str) -> Option<Self> {
        serde_json::from_str(text).ok()
    }
}

/// The audio format the client declares in `session.start`: always 16 kHz PCM s16le.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InputAudio {
    pub encoding: Encoding,
    pub sample_rate: InputRate,
}

/// The only input sample rate, 16 kHz; any other number is refused when parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct InputRate;

impl TryFrom<u32> for InputRate {
    type Error = String;

    fn try_from(rate: u32) -> Result<Self, Self::Error> {
        if rate == INPUT_SAMPLE_RATE {
            Ok(Self)
        } else {
            Err(format!("the input sample rate is {INPUT_SAMPLE_RATE}"))
        }
    }
}

impl From<InputRate> for u32 {
    fn from(_: InputRate) -> Self {
        INPUT_SAMPLE_RATE
    }
}

/// Sample encoding of audio frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Encoding {
    /// Signed 16-bit little-endian PCM, mono.
    #[serde(rename = "pcm_s16le")]
    PcmS16le,
}

/// A text frame from the server.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum ServerEvent {
    /// `session.started`: the first event, with the audio formats.
    #[serde(rename = "session.started")]
    SessionStarted {
        session_id: String,
        audio: AudioFormats,
    },
    /// `transcript.partial`: an interim caller transcript.
    #[serde(rename = "transcript.partial")]
    TranscriptPartial {
        turn_id: u32,
        speaker: Speaker,
        text: SensitiveText,
    },
    /// `transcript.final`: the final text of a turn; for the agent, what was spoken.
    #[serde(rename = "transcript.final")]
    TranscriptFinal {
        turn_id: u32,
        speaker: Speaker,
        text: SensitiveText,
        interrupted: bool,
    },
    /// `agent.audio.start`: the frames that follow belong to `turn_id`.
    #[serde(rename = "agent.audio.start")]
    AgentAudioStart { turn_id: u32 },
    /// `agent.audio.end`: every frame of `turn_id` was sent.
    #[serde(rename = "agent.audio.end")]
    AgentAudioEnd { turn_id: u32 },
    /// `agent.interrupted`: barge-in; the client drops queued audio.
    #[serde(rename = "agent.interrupted")]
    AgentInterrupted { turn_id: u32 },
    /// `plan.node`: the conversation moved to a plan node (M4b).
    #[serde(rename = "plan.node")]
    PlanNode { turn_id: u32, node_id: String },
    /// `decision`: a decision-model answer about caller turn `turn_id`.
    #[serde(rename = "decision")]
    Decision {
        turn_id: u32,
        question: String,
        status: DecisionStatus,
        latency_ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        probability: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        choice: Option<String>,
    },
    /// `latency`: the stage timings of agent reply `turn_id`.
    #[serde(rename = "latency")]
    Latency {
        turn_id: u32,
        stt_ms: Option<u64>,
        llm_ttft_ms: Option<u64>,
        tts_first_audio_ms: Option<u64>,
        mouth_to_ear_ms: Option<u64>,
    },
    /// `error`: something went wrong; the session goes on unless `session.ended` follows.
    #[serde(rename = "error")]
    Error { code: ErrorCode, message: String },
    /// `session.ended`: the last event.
    #[serde(rename = "session.ended")]
    SessionEnded {
        reason: EndReason,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opt_out_layer: Option<OptOutLayer>,
    },
}

impl ServerEvent {
    /// `session.started` for `session_id`, with the protocol's audio formats.
    #[must_use]
    pub fn session_started(session_id: impl Into<String>) -> Self {
        let format = |sample_rate| AudioFormat {
            encoding: Encoding::PcmS16le,
            sample_rate,
            frame_ms: FRAME_MS,
        };
        Self::SessionStarted {
            session_id: session_id.into(),
            audio: AudioFormats {
                input: format(INPUT_SAMPLE_RATE),
                output: format(OUTPUT_SAMPLE_RATE),
            },
        }
    }

    /// `session.ended` for `reason`, naming the opt-out layer if there is one.
    #[must_use]
    pub fn session_ended(reason: SessionEndReason) -> Self {
        let (reason, opt_out_layer) = match reason {
            SessionEndReason::ClientEnded => (EndReason::ClientEnded, None),
            SessionEndReason::OptOut(layer) => (EndReason::OptOut, Some(layer.into())),
            SessionEndReason::Escalation => (EndReason::Escalation, None),
            SessionEndReason::EndNode => (EndReason::EndNode, None),
            SessionEndReason::Timeout => (EndReason::Timeout, None),
            SessionEndReason::Error => (EndReason::Error, None),
        };
        Self::SessionEnded {
            reason,
            opt_out_layer,
        }
    }

    /// The event as a JSON text frame.
    #[must_use]
    pub fn to_json(&self) -> String {
        // Every field serializes (strings, numbers, booleans): this cannot fail.
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// The audio formats of both directions, as announced in `session.started`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AudioFormats {
    pub input: AudioFormat,
    pub output: AudioFormat,
}

/// One direction's audio format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AudioFormat {
    pub encoding: Encoding,
    pub sample_rate: u32,
    pub frame_ms: u32,
}

/// Who spoke a turn, on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speaker {
    Caller,
    Agent,
}

impl From<DomainSpeaker> for Speaker {
    fn from(speaker: DomainSpeaker) -> Self {
        match speaker {
            DomainSpeaker::Caller => Self::Caller,
            DomainSpeaker::Agent => Self::Agent,
        }
    }
}

/// How a decision call went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Answered,
    Timeout,
    Error,
}

/// The `code` of an `error` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// A text frame that is not a client event.
    BadMessage,
    /// Audio the server can't use.
    UnsupportedAudio,
    /// A model failed; the turn was skipped.
    ProviderUnavailable,
    Internal,
}

/// Why the session ended, on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    ClientEnded,
    OptOut,
    Escalation,
    EndNode,
    Timeout,
    Error,
}

/// Which opt-out layer ended the session, on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptOutLayer {
    Phrase,
    Decision,
    LlmTool,
}

impl From<vak_domain::OptOutLayer> for OptOutLayer {
    fn from(layer: vak_domain::OptOutLayer) -> Self {
        match layer {
            vak_domain::OptOutLayer::Phrase => Self::Phrase,
            vak_domain::OptOutLayer::Decision => Self::Decision,
            vak_domain::OptOutLayer::LlmTool => Self::LlmTool,
        }
    }
}

/// What reaches the pipeline from the client.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Inbound {
    /// A binary frame: 16 kHz mono PCM16 samples.
    Audio(Vec<i16>),
    /// A text frame that parsed as a client event.
    Event(ClientEvent),
    /// A text frame that is not a client event.
    Invalid,
    /// A binary frame with an odd number of bytes: not PCM16.
    UnsupportedAudio,
}

/// What the pipeline sends to the client.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
#[expect(
    clippy::large_enum_variant,
    reason = "frames move by value, so sending one never allocates"
)]
pub enum Outbound {
    Event(ServerEvent),
    /// One 20 ms frame of agent audio at [`OUTPUT_SAMPLE_RATE`].
    Audio(AudioFrame),
}

/// The transport is closed: the other end is gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the media transport is closed")]
pub struct Closed;

/// The pipeline's end of a session's media channels.
#[derive(Debug)]
pub struct MediaTransport {
    inbound: mpsc::Receiver<Inbound>,
    outbound: mpsc::Sender<Outbound>,
}

/// The client's end of a session's media channels: the WebSocket bridge or a fake client.
#[derive(Debug)]
pub struct MediaPeer {
    sender: PeerSender,
    outbound: mpsc::Receiver<Outbound>,
}

/// What the client side uses to reach the pipeline. Cheap to clone.
#[derive(Debug, Clone)]
pub struct PeerSender {
    inbound: mpsc::Sender<Inbound>,
}

/// What became of a pushed audio frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushAudio {
    Accepted,
    /// The pipeline is behind; the frame was dropped.
    Dropped,
    Closed,
}

/// A connected transport and peer.
#[must_use]
pub fn media_channel() -> (MediaTransport, MediaPeer) {
    let (inbound_tx, inbound_rx) = mpsc::channel(INBOUND_CAPACITY);
    let (outbound_tx, outbound_rx) = mpsc::channel(OUTBOUND_CAPACITY);
    (
        MediaTransport {
            inbound: inbound_rx,
            outbound: outbound_tx,
        },
        MediaPeer {
            sender: PeerSender {
                inbound: inbound_tx,
            },
            outbound: outbound_rx,
        },
    )
}

impl MediaTransport {
    /// The next thing the client sent; `None` once the client is gone.
    pub async fn recv(&mut self) -> Option<Inbound> {
        self.inbound.recv().await
    }

    /// Sends to the client, waiting while its queue is full.
    pub async fn send(&self, item: Outbound) -> Result<(), Closed> {
        self.outbound.send(item).await.map_err(|_| Closed)
    }

    /// The receiver of client input and the sender of server output, to use apart.
    #[must_use]
    pub fn into_parts(self) -> (mpsc::Receiver<Inbound>, mpsc::Sender<Outbound>) {
        (self.inbound, self.outbound)
    }
}

impl MediaPeer {
    /// The sender half and the receiver of everything the pipeline sends.
    #[must_use]
    pub fn into_parts(self) -> (PeerSender, mpsc::Receiver<Outbound>) {
        (self.sender, self.outbound)
    }
}

impl PeerSender {
    /// A binary frame of 16 kHz samples. Never waits: a full queue drops the frame.
    #[must_use]
    pub fn push_audio(&self, samples: Vec<i16>) -> PushAudio {
        match self.inbound.try_send(Inbound::Audio(samples)) {
            Ok(()) => PushAudio::Accepted,
            Err(mpsc::error::TrySendError::Full(_)) => PushAudio::Dropped,
            Err(mpsc::error::TrySendError::Closed(_)) => PushAudio::Closed,
        }
    }

    /// A binary frame as bytes: PCM s16le, or `UnsupportedAudio` for an odd length.
    #[must_use]
    pub fn push_audio_bytes(&self, bytes: &[u8]) -> PushAudio {
        let (pairs, rest) = bytes.as_chunks::<2>();
        if !rest.is_empty() {
            return match self.inbound.try_send(Inbound::UnsupportedAudio) {
                Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => PushAudio::Dropped,
                Err(mpsc::error::TrySendError::Closed(_)) => PushAudio::Closed,
            };
        }
        self.push_audio(pairs.iter().map(|pair| i16::from_le_bytes(*pair)).collect())
    }

    /// A text frame: a client event, or an invalid message the pipeline answers with an
    /// error.
    pub async fn push_text(&self, text: &str) -> Result<(), Closed> {
        let item = ClientEvent::from_json(text).map_or(Inbound::Invalid, Inbound::Event);
        self.inbound.send(item).await.map_err(|_| Closed)
    }

    /// A client event.
    pub async fn push_event(&self, event: ClientEvent) -> Result<(), Closed> {
        self.inbound
            .send(Inbound::Event(event))
            .await
            .map_err(|_| Closed)
    }

    /// Whether the pipeline is gone.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.inbound.is_closed()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn the_peer_reaches_the_transport_and_back() {
        let (mut transport, peer) = media_channel();
        let (sender, mut outbound) = peer.into_parts();
        assert_eq!(sender.push_audio(vec![1, 2]), PushAudio::Accepted);
        assert_eq!(
            sender.push_audio_bytes(&[1, 0, 255, 255]),
            PushAudio::Accepted
        );
        sender.push_text(r#"{"type":"session.end"}"#).await.unwrap();
        sender.push_text("not json").await.unwrap();
        sender
            .push_event(ClientEvent::Mute { muted: true })
            .await
            .unwrap();
        assert_eq!(transport.recv().await, Some(Inbound::Audio(vec![1, 2])));
        assert_eq!(transport.recv().await, Some(Inbound::Audio(vec![1, -1])));
        assert_eq!(
            transport.recv().await,
            Some(Inbound::Event(ClientEvent::SessionEnd))
        );
        assert_eq!(transport.recv().await, Some(Inbound::Invalid));
        assert_eq!(
            transport.recv().await,
            Some(Inbound::Event(ClientEvent::Mute { muted: true }))
        );

        let event = ServerEvent::AgentAudioStart { turn_id: 3 };
        transport
            .send(Outbound::Event(event.clone()))
            .await
            .unwrap();
        assert_eq!(outbound.recv().await, Some(Outbound::Event(event)));
        assert!(!sender.is_closed());
        drop(transport);
        assert!(sender.is_closed());
        assert_eq!(sender.push_audio(vec![0]), PushAudio::Closed);
        assert!(sender.push_text("{}").await.is_err());
        assert!(sender.push_event(ClientEvent::SessionEnd).await.is_err());
    }

    #[tokio::test]
    async fn late_audio_is_dropped_and_odd_bytes_are_unsupported() {
        let (mut transport, peer) = media_channel();
        let (sender, _outbound) = peer.into_parts();
        for _ in 0..INBOUND_CAPACITY {
            assert_eq!(sender.push_audio(vec![0]), PushAudio::Accepted);
        }
        assert_eq!(sender.push_audio(vec![0]), PushAudio::Dropped);
        assert_eq!(sender.push_audio_bytes(&[0, 0]), PushAudio::Dropped);
        assert_eq!(sender.push_audio_bytes(&[0]), PushAudio::Dropped);
        while transport.inbound.try_recv().is_ok() {}
        assert_eq!(sender.push_audio_bytes(&[0, 0, 0]), PushAudio::Dropped);
        assert_eq!(transport.recv().await, Some(Inbound::UnsupportedAudio));
        drop(transport);
        assert_eq!(sender.push_audio_bytes(&[0]), PushAudio::Closed);
    }

    #[test]
    fn server_events_have_their_wire_shape() {
        let started: serde_json::Value =
            serde_json::from_str(&ServerEvent::session_started("s1").to_json()).unwrap();
        assert_eq!(
            started,
            json!({"type": "session.started", "session_id": "s1", "audio": {
                "input": {"encoding": "pcm_s16le", "sample_rate": 16000, "frame_ms": 20},
                "output": {"encoding": "pcm_s16le", "sample_rate": 24000, "frame_ms": 20}}})
        );
        let opted_out =
            ServerEvent::session_ended(SessionEndReason::OptOut(vak_domain::OptOutLayer::LlmTool));
        assert_eq!(
            opted_out.to_json(),
            r#"{"type":"session.ended","reason":"opt_out","opt_out_layer":"llm_tool"}"#
        );
        for (reason, wire) in [
            (SessionEndReason::ClientEnded, "client_ended"),
            (SessionEndReason::Escalation, "escalation"),
            (SessionEndReason::EndNode, "end_node"),
            (SessionEndReason::Timeout, "timeout"),
            (SessionEndReason::Error, "error"),
        ] {
            assert_eq!(
                ServerEvent::session_ended(reason).to_json(),
                format!(r#"{{"type":"session.ended","reason":"{wire}"}}"#)
            );
        }
        for (layer, wire) in [
            (vak_domain::OptOutLayer::Phrase, OptOutLayer::Phrase),
            (vak_domain::OptOutLayer::Decision, OptOutLayer::Decision),
        ] {
            assert_eq!(OptOutLayer::from(layer), wire);
        }
        assert_eq!(Speaker::from(DomainSpeaker::Agent), Speaker::Agent);
        assert_eq!(Speaker::from(DomainSpeaker::Caller), Speaker::Caller);
        assert_eq!(u32::from(InputRate), INPUT_SAMPLE_RATE);
    }
}
