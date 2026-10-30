//! HTTP surface: `GET /health` and `POST /serving-endpoints/{stt,tts}/invocations`.
//! Each invocation is logged with its endpoint, status and latency, never its payload.

use std::sync::Arc;
use std::time::Instant;

use axum::Router;
use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Serialize;
use serde_json::{Value, json};

use crate::error::ServingError;
use crate::mlx_audio::MlxAudio;
use crate::request::{self, PCM_S16LE, TTS_SAMPLE_RATE};

/// Databricks' payload limit is 16 MB per request.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

pub(crate) fn router(mlx_audio: Arc<MlxAudio>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/serving-endpoints/{name}/invocations", post(invoke))
        .fallback(|| async { ServingError::not_found("no such route") })
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(mlx_audio)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

#[derive(Serialize)]
struct Predictions<T> {
    predictions: Vec<T>,
}

#[derive(Serialize)]
struct Transcript {
    text: String,
}

#[derive(Serialize)]
struct Speech {
    audio_b64: String,
    encoding: &'static str,
    sample_rate: u32,
}

async fn invoke(
    State(mlx_audio): State<Arc<MlxAudio>>,
    Path(name): Path<String>,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let started = Instant::now();
    let (endpoint, result) = match name.as_str() {
        "stt" => ("stt", stt(&mlx_audio, body).await),
        "tts" => ("tts", tts(&mlx_audio, body).await),
        _ => (
            "unknown",
            Err(ServingError::not_found(
                "no such serving endpoint; this shim serves `stt` and `tts`",
            )),
        ),
    };
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    match result {
        Ok(response) => {
            tracing::info!(endpoint, status = 200, elapsed_ms, "invocation");
            response
        }
        Err(error) => {
            tracing::warn!(
                endpoint,
                status = error.status().as_u16(),
                error_code = ?error.code(),
                elapsed_ms,
                "invocation failed"
            );
            error.into_response()
        }
    }
}

async fn stt(
    mlx_audio: &MlxAudio,
    body: Result<Bytes, BytesRejection>,
) -> Result<Response, ServingError> {
    let records = request::stt_records(&body.map_err(|rejection| body_error(&rejection))?)?;
    let mut predictions = Vec::with_capacity(records.len());
    for record in records {
        predictions.push(Transcript {
            text: mlx_audio.transcribe(record).await?,
        });
    }
    Ok(Json(Predictions { predictions }).into_response())
}

async fn tts(
    mlx_audio: &MlxAudio,
    body: Result<Bytes, BytesRejection>,
) -> Result<Response, ServingError> {
    let records = request::tts_records(&body.map_err(|rejection| body_error(&rejection))?)?;
    let mut predictions = Vec::with_capacity(records.len());
    for record in &records {
        predictions.push(Speech {
            audio_b64: BASE64.encode(mlx_audio.synthesize(record).await?),
            encoding: PCM_S16LE,
            sample_rate: TTS_SAMPLE_RATE,
        });
    }
    Ok(Json(Predictions { predictions }).into_response())
}

fn body_error(rejection: &BytesRejection) -> ServingError {
    if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ServingError::too_large("the body is over 16 MiB")
    } else {
        ServingError::bad_request("the body could not be read")
    }
}
