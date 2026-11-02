//! `GET /api/v1/voice/latency`: the voice path's stage latencies, for the dev view.

use axum::Json;
use axum::extract::State;
use serde::Serialize;
use utoipa::ToSchema;
use vak_services::latency::{Stage, StageReport};

use crate::problem::Problem;
use crate::state::AppState;

/// A stage of the voice path.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = VoiceStage)]
pub(crate) enum StageDto {
    /// Speech-to-text: from the end of the caller's turn to its transcript.
    Stt,
    /// LLM time to first token.
    LlmTtft,
    /// TTS time to the first clause's audio.
    TtsFirstAudio,
    /// From the end of the caller's speech to the reply's first audio.
    MouthToEar,
}

impl From<Stage> for StageDto {
    fn from(stage: Stage) -> Self {
        match stage {
            Stage::Stt => Self::Stt,
            Stage::LlmTtft => Self::LlmTtft,
            Stage::TtsFirstAudio => Self::TtsFirstAudio,
            Stage::MouthToEar => Self::MouthToEar,
        }
    }
}

/// Replies whose stage took at most `leMs` (and more than the previous bucket's bound).
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = LatencyBucket)]
pub(crate) struct BucketDto {
    /// The bucket's upper bound in milliseconds; `null` for the slowest bucket.
    le_ms: Option<u64>,
    count: u64,
}

/// One stage's histogram.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = StageLatency)]
pub(crate) struct StageLatencyDto {
    stage: StageDto,
    /// Replies measured since the server started.
    count: u64,
    /// Median over the last 1000 replies; `null` before the first.
    p50_ms: Option<u64>,
    /// 95th percentile over the last 1000 replies; `null` before the first.
    p95_ms: Option<u64>,
    buckets: Vec<BucketDto>,
}

impl From<StageReport> for StageLatencyDto {
    fn from(report: StageReport) -> Self {
        Self {
            stage: report.stage.into(),
            count: report.count,
            p50_ms: report.p50_ms,
            p95_ms: report.p95_ms,
            buckets: report
                .buckets
                .into_iter()
                .map(|bucket| BucketDto {
                    le_ms: bucket.le_ms,
                    count: bucket.count,
                })
                .collect(),
        }
    }
}

/// Every stage, in pipeline order.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = VoiceLatency)]
pub(crate) struct VoiceLatencyDto {
    stages: Vec<StageLatencyDto>,
}

/// The voice path's latency so far: one histogram per stage, since the server started.
#[utoipa::path(
    get,
    path = "/voice/latency",
    operation_id = "getVoiceLatency",
    tag = "voice",
    responses(
        (status = OK, description = "One histogram per stage", body = VoiceLatencyDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn get(State(state): State<AppState>) -> Json<VoiceLatencyDto> {
    Json(VoiceLatencyDto {
        stages: state
            .voice
            .latency()
            .into_iter()
            .map(StageLatencyDto::from)
            .collect(),
    })
}
