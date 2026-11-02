//! The WebSocket ↔ media transport bridge: one task per socket that moves text and binary
//! frames between the browser and the session's pipeline, and does nothing else, so the
//! pipeline's sends never wait on more than the socket.
//!
//! Client audio is pushed without waiting: if the pipeline falls behind, frames are dropped
//! (late audio is worthless) rather than stalling the socket reader.

use axum::extract::ws::{Message, WebSocket};
use vak_services::MediaSession;
use vak_services::media::{Outbound, PushAudio, media_channel};

pub(crate) async fn run(mut socket: WebSocket, session: MediaSession) {
    let (transport, peer) = media_channel();
    let (sender, mut outbound) = peer.into_parts();
    let session = tokio::spawn(session.run(transport));
    let mut dropped: u64 = 0;
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Binary(bytes))) => match sender.push_audio_bytes(&bytes) {
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
                        let bytes: Vec<u8> = frame
                            .samples()
                            .iter()
                            .flat_map(|sample| sample.to_le_bytes())
                            .collect();
                        Message::Binary(bytes.into())
                    }
                    Some(_) => continue,
                    // The session is over and has said so.
                    None => break,
                };
                if socket.send(message).await.is_err() {
                    break;
                }
            }
        }
    }
    // Hanging up the transport ends the session if the client left first.
    drop(sender);
    drop(outbound);
    let _ = socket.send(Message::Close(None)).await;
    match session.await {
        Ok(reason) => tracing::info!(?reason, dropped_frames = dropped, "media socket closed"),
        Err(error) => tracing::error!(%error, "the voice session task failed"),
    }
}
