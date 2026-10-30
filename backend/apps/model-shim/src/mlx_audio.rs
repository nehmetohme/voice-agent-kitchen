//! The mlx-audio client: OpenAI-style `/v1/audio/transcriptions` (multipart) and
//! `/v1/audio/speech` (JSON). Every call has the configured deadline; failures map to
//! Databricks-shaped errors.

use std::time::Duration;

use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::ServingError;
use crate::request::{STT_SAMPLE_RATE, SttRecord, TTS_SAMPLE_RATE, TtsRecord};
use crate::wav;

/// Connecting to a local server is instant; failing fast tells "down" from "slow".
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) struct MlxAudio {
    client: reqwest::Client,
    transcriptions_url: String,
    speech_url: String,
    stt_model: String,
    tts_model: String,
}

#[derive(Deserialize)]
struct Transcription {
    text: String,
}

#[derive(Serialize)]
struct SpeechRequest<'a> {
    model: &'a str,
    input: &'a str,
    voice: &'a str,
    speed: f64,
    lang_code: &'a str,
    response_format: &'static str,
}

impl MlxAudio {
    pub(crate) fn new(config: &Config) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .connect_timeout(CONNECT_TIMEOUT.min(config.timeout))
            .build()?;
        Ok(Self {
            client,
            transcriptions_url: format!("{}/v1/audio/transcriptions", config.mlx_audio_url),
            speech_url: format!("{}/v1/audio/speech", config.mlx_audio_url),
            stt_model: config.stt_model.clone(),
            tts_model: config.tts_model.clone(),
        })
    }

    /// The trimmed transcript of one utterance.
    pub(crate) async fn transcribe(&self, record: SttRecord) -> Result<String, ServingError> {
        let audio = wav::pcm16_mono(STT_SAMPLE_RATE, &record.pcm)
            .ok_or_else(|| ServingError::bad_request("the audio is too long"))?;
        let mut form = Form::new()
            .text("model", self.stt_model.clone())
            .text("response_format", "json")
            .part("file", Part::bytes(audio).file_name("audio.wav"));
        if let Some(language) = record.language {
            form = form.text("language", language);
        }
        let response = self
            .client
            .post(&self.transcriptions_url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| upstream_error("STT", &e))?;
        let transcription: Transcription = check_status("STT", response)?
            .json()
            .await
            .map_err(|e| upstream_error("STT", &e))?;
        Ok(transcription.text.trim().to_owned())
    }

    /// PCM s16le mono at 24 kHz for one clause.
    pub(crate) async fn synthesize(&self, record: &TtsRecord) -> Result<Vec<u8>, ServingError> {
        let request = SpeechRequest {
            model: &self.tts_model,
            input: &record.text,
            voice: &record.voice,
            speed: record.speed,
            lang_code: record.lang_code(),
            response_format: "wav",
        };
        let response = self
            .client
            .post(&self.speech_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| upstream_error("TTS", &e))?;
        let bytes = check_status("TTS", response)?
            .bytes()
            .await
            .map_err(|e| upstream_error("TTS", &e))?;
        let audio = wav::parse(&bytes)
            .map_err(|reason| ServingError::internal(format!("mlx-audio TTS answered {reason}")))?;
        let format = (audio.format_tag, audio.channels, audio.bits_per_sample);
        if format != (1, 1, 16) || audio.sample_rate != TTS_SAMPLE_RATE {
            return Err(ServingError::internal(format!(
                "mlx-audio TTS answered {} Hz, {} channel(s), {}-bit (format {}); the contract \
                 needs 24000 Hz mono 16-bit PCM: check VAK_SHIM_TTS_MODEL",
                audio.sample_rate, audio.channels, audio.bits_per_sample, audio.format_tag
            )));
        }
        if audio.data.is_empty() || audio.data.len() % 2 != 0 {
            return Err(ServingError::internal(
                "mlx-audio TTS answered no whole samples",
            ));
        }
        Ok(audio.data.to_vec())
    }
}

/// A non-success status is the model failing: 500. Its body is not read, since it may
/// quote the input.
fn check_status(
    what: &str,
    response: reqwest::Response,
) -> Result<reqwest::Response, ServingError> {
    let status = response.status();
    if status.is_success() {
        Ok(response)
    } else {
        Err(ServingError::internal(format!(
            "mlx-audio {what} answered HTTP {status}; see the mlx-audio log"
        )))
    }
}

/// Timeouts are 504, connection failures 503, anything else (an unusable answer) 500.
fn upstream_error(what: &str, error: &reqwest::Error) -> ServingError {
    if error.is_timeout() {
        ServingError::deadline_exceeded(format!("mlx-audio {what} did not answer in time"))
    } else if error.is_connect() || error.is_request() {
        ServingError::unavailable(format!("mlx-audio {what} is unreachable; is it running?"))
    } else {
        ServingError::internal(format!("mlx-audio {what} answered something unusable"))
    }
}
