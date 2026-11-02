//! The media transport port and the `vak.media.v1` events (`contracts/media/`).
//!
//! A session's transport is a pair of bounded channels: [`MediaTransport`] is the pipeline's
//! end, [`MediaPeer`] the other end, held by the browser WebSocket bridge (`vak-api`) or a
//! test's fake client (`vak-testkit`). Text frames are [`ClientEvent`] and [`ServerEvent`]
//! as JSON; binary frames are PCM16 audio, 16 kHz in and 24 kHz out, in 20 ms frames.

use tokio::sync::mpsc;
use vak_audio::AudioFrame;

use crate::SensitiveText;

/// The WebSocket subprotocol.
pub const PROTOCOL: &str = "vak.media.v1";
/// Sample rate of the client's audio (PCM s16le mono).
pub const INPUT_SAMPLE_RATE: u32 = 16_000;
/// Sample rate of the server's audio (PCM s16le mono): Kokoro's native rate.
pub const OUTPUT_SAMPLE_RATE: u32 = 24_000;
/// Length of an audio frame in either direction.
pub const FRAME_MS: u32 = vak_audio::FRAME_MS;

/// The `type` of every event a client may send.
// CONTRACT-STUB
pub const CLIENT_EVENT_TYPES: &[&str] = &[];
/// The `type` of every event the server may send.
// CONTRACT-STUB
pub const SERVER_EVENT_TYPES: &[&str] = &[];

/// A text frame from the client.
// CONTRACT-STUB: the wire names (`#[serde(rename = …)]`) arrive with the implementation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum ClientEvent {
    /// `session.start`: the client is ready to talk.
    SessionStart { audio: Option<InputAudio> },
    /// `client.mute`: the microphone is muted or unmuted.
    Mute { muted: bool },
    /// `session.end`: the caller hung up.
    SessionEnd,
}

/// The audio format the client declares in `session.start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InputAudio {
    pub encoding: Encoding,
    pub sample_rate: u32,
}

/// Sample encoding of audio frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Encoding {
    /// Signed 16-bit little-endian PCM, mono.
    PcmS16le,
}

/// A text frame from the server.
// CONTRACT-STUB: the wire names (`#[serde(rename = …)]`) arrive with the implementation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum ServerEvent {
    /// `session.started`: the first event, with the audio formats.
    SessionStarted {
        session_id: String,
        audio: AudioFormats,
    },
    /// `transcript.partial`: an interim caller transcript.
    TranscriptPartial {
        turn_id: u32,
        speaker: Speaker,
        text: SensitiveText,
    },
    /// `transcript.final`: the final text of a turn; for the agent, what was spoken.
    TranscriptFinal {
        turn_id: u32,
        speaker: Speaker,
        text: SensitiveText,
        interrupted: bool,
    },
    /// `agent.audio.start`: the frames that follow belong to `turn_id`.
    AgentAudioStart { turn_id: u32 },
    /// `agent.audio.end`: every frame of `turn_id` was sent.
    AgentAudioEnd { turn_id: u32 },
    /// `agent.interrupted`: barge-in; the client drops queued audio.
    AgentInterrupted { turn_id: u32 },
    /// `plan.node`: the conversation moved to a plan node (M4b).
    PlanNode { turn_id: u32, node_id: String },
    /// `decision`: a decision-model answer about caller turn `turn_id`.
    Decision {
        turn_id: u32,
        question: String,
        status: DecisionStatus,
        latency_ms: u64,
        probability: Option<f64>,
        choice: Option<String>,
    },
    /// `latency`: the stage timings of agent reply `turn_id`.
    Latency {
        turn_id: u32,
        stt_ms: Option<u64>,
        llm_ttft_ms: Option<u64>,
        tts_first_audio_ms: Option<u64>,
        mouth_to_ear_ms: Option<u64>,
    },
    /// `error`: something went wrong; the session goes on unless `session.ended` follows.
    Error { code: ErrorCode, message: String },
    /// `session.ended`: the last event.
    SessionEnded {
        reason: EndReason,
        opt_out_layer: Option<OptOutLayer>,
    },
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
pub enum Speaker {
    Caller,
    Agent,
}

/// How a decision call went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DecisionStatus {
    Answered,
    Timeout,
    Error,
}

/// The `code` of an `error` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
pub enum OptOutLayer {
    Phrase,
    Decision,
    LlmTool,
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

/// The pipeline's end of a session's media channels.
#[derive(Debug)]
pub struct MediaTransport {
    inbound: mpsc::Receiver<Inbound>,
    outbound: mpsc::Sender<Outbound>,
}

/// The client's end of a session's media channels: the WebSocket bridge or a fake client.
#[derive(Debug)]
pub struct MediaPeer {
    inbound: mpsc::Sender<Inbound>,
    outbound: mpsc::Receiver<Outbound>,
}

/// A connected transport and peer.
#[must_use]
pub fn media_channel() -> (MediaTransport, MediaPeer) {
    // CONTRACT-STUB: capacities and overflow policies arrive with the implementation.
    let (inbound_tx, inbound_rx) = mpsc::channel(1);
    let (outbound_tx, outbound_rx) = mpsc::channel(1);
    (
        MediaTransport {
            inbound: inbound_rx,
            outbound: outbound_tx,
        },
        MediaPeer {
            inbound: inbound_tx,
            outbound: outbound_rx,
        },
    )
}

impl MediaTransport {
    /// The receiver of client input and the sender of server output.
    #[must_use]
    pub fn into_parts(self) -> (mpsc::Receiver<Inbound>, mpsc::Sender<Outbound>) {
        (self.inbound, self.outbound)
    }
}

impl MediaPeer {
    /// The sender of client input and the receiver of server output.
    #[must_use]
    pub fn into_parts(self) -> (mpsc::Sender<Inbound>, mpsc::Receiver<Outbound>) {
        (self.inbound, self.outbound)
    }
}
