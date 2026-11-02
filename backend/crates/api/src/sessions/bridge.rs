//! The WebSocket ↔ media transport bridge: one task per socket that moves text and binary
//! frames between the browser and the session's pipeline, and does nothing else, so the
//! pipeline's sends never wait on more than the socket.
//!
//! Client audio is pushed without waiting: if the pipeline falls behind, frames are dropped
//! (late audio is worthless) rather than stalling the socket reader. Audio never allocates per
//! frame: an `InputFramer` cuts client bytes into fixed frames, and agent frames are written
//! into one reused buffer.

use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use bytes::BytesMut;
use vak_services::MediaSession;
use vak_services::media::{InputFramer, Outbound, PushAudio, media_channel};

/// How long one write to the socket may take. A client that stops reading (a full TCP
/// window) is gone: without this the bridge would wait, never see the close, and the session
/// would hold its slot until its time limit.
const SEND_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) async fn run(mut socket: WebSocket, session: MediaSession) {
    let (transport, peer) = media_channel();
    let (sender, mut outbound) = peer.into_parts();
    let session = tokio::spawn(session.run(transport));
    let mut framer = InputFramer::default();
    let mut audio = BytesMut::new();
    let mut dropped: u64 = 0;
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Binary(bytes))) => match framer.push(&sender, &bytes) {
                    PushAudio::Accepted => {}
                    PushAudio::Dropped => dropped += 1,
                    PushAudio::Closed => break,
                },
                Some(Ok(Message::Text(text))) => {
                    if sender.push_text(text.as_str()).await.is_err() {
                        break;
                    }
                }
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
            },
            item = outbound.recv() => {
                let message = match item {
                    Some(Outbound::Event(event)) => Message::Text(event.to_json().into()),
                    Some(Outbound::Audio(frame)) => {
                        // `split` hands the bytes to the socket; once it has written them, the
                        // next `resize` reclaims the same allocation.
                        audio.resize(frame.byte_len(), 0);
                        frame.copy_le_bytes(&mut audio);
                        Message::Binary(audio.split().freeze())
                    }
                    Some(_) => continue,
                    // The session is over and has said so.
                    None => break,
                };
                if !matches!(tokio::time::timeout(SEND_TIMEOUT, socket.send(message)).await, Ok(Ok(()))) {
                    break;
                }
            }
        }
    }
    // Hanging up the transport ends the session if the client left first.
    drop(sender);
    drop(outbound);
    // A courtesy close: the client may already be gone, so a failure changes nothing.
    let _ = tokio::time::timeout(SEND_TIMEOUT, socket.send(Message::Close(None))).await;
    match session.await {
        Ok(reason) => tracing::info!(?reason, dropped_frames = dropped, "media socket closed"),
        Err(error) => tracing::error!(%error, "the voice session task failed"),
    }
}
