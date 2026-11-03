//! The voice engines: one adapter per model, built from the model settings. Local and
//! Databricks differ only in those settings.

use std::sync::Arc;

use anyhow::Context as _;
use vak_adapter_decision_systemone::{SystemOneConfig, SystemOneDecisions};
use vak_adapter_llm_openai::{OpenAiLlm, OpenAiLlmConfig};
use vak_adapter_stt_serving::{ServingStt, ServingSttConfig};
use vak_adapter_tts_serving::{ServingTts, ServingTtsConfig};
use vak_config::{LLM_EXTRA_BODY_VAR, ModelSettings};
use vak_services::Engines;

/// The engines, or `None` (voice sessions off) when a model is not configured.
pub(crate) fn engines(models: &ModelSettings) -> anyhow::Result<Option<Engines>> {
    let (Some(llm), Some(stt), Some(tts), Some(decisions)) =
        (&models.llm, &models.stt, &models.tts, &models.decisions)
    else {
        if let Some(missing) = models.first_missing() {
            tracing::warn!(missing, "voice sessions are off: a model is not configured");
        }
        return Ok(None);
    };

    let mut llm_config = OpenAiLlmConfig::new(&llm.base_url, &llm.model);
    if let Some(key) = &llm.api_key {
        llm_config = llm_config.with_bearer_token(key.clone());
    }
    if let Some(extra) = &llm.extra_body {
        let extra = serde_json::from_str(extra)
            .with_context(|| format!("{LLM_EXTRA_BODY_VAR} is not JSON"))?;
        llm_config = llm_config.with_extra_body(extra);
    }
    let llm_engine = OpenAiLlm::new(llm_config).context("configuring the LLM")?;
    let stt_engine = ServingStt::new(ServingSttConfig::new(&stt.base_url, &stt.endpoint))
        .context("configuring speech-to-text")?;
    let tts_engine = ServingTts::new(ServingTtsConfig::new(&tts.base_url, &tts.endpoint))
        .context("configuring text-to-speech")?;
    let mut decision_config = SystemOneConfig::new(&decisions.base_url, &decisions.model);
    if let Some(path) = &decisions.path {
        decision_config = decision_config.with_path(path);
    }
    let decision_engine =
        SystemOneDecisions::new(decision_config).context("configuring the decision model")?;

    tracing::info!(llm = %llm.model, decisions = %decisions.model, "voice sessions on");
    Ok(Some(Engines::new(
        Arc::new(stt_engine),
        Arc::new(llm_engine),
        Arc::new(tts_engine),
        Arc::new(decision_engine),
    )))
}

#[cfg(test)]
mod tests {
    use vak_config::Settings;

    use super::*;

    fn models(vars: &[(&str, &str)]) -> ModelSettings {
        Settings::from_lookup(|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        })
        .expect("valid settings")
        .models
    }

    const ALL: [(&str, &str); 7] = [
        ("VAK_LLM_BASE_URL", "http://127.0.0.1:8080/v1"),
        ("VAK_LLM_MODEL", "local"),
        ("VAK_STT_BASE_URL", "http://127.0.0.1:8001"),
        ("VAK_TTS_BASE_URL", "http://127.0.0.1:8001"),
        ("VAK_DECISION_BASE_URL", "http://127.0.0.1:8081"),
        ("VAK_DECISION_MODEL", "Kev-4B"),
        (
            "VAK_LLM_EXTRA_BODY",
            r#"{"chat_template_kwargs":{"enable_thinking":false}}"#,
        ),
    ];

    #[test]
    fn every_model_configured_turns_voice_on() {
        assert!(engines(&models(&ALL)).expect("valid").is_some());
        let mut keyed = ALL.to_vec();
        keyed.push(("VAK_LLM_API_KEY", "sk-test"));
        keyed.push(("VAK_DECISION_PATH", "/ai-gateway/typesafe/v1/systemone"));
        assert!(engines(&models(&keyed)).expect("valid").is_some());
    }

    /// Locally the TTS (mlx-audio, offline) loads a voice only from the Kokoro model folder
    /// that `deploy/local/models.sh` fills, so every voice an agent can pick must be pinned
    /// there, beside the weights; a missing one fails the call before the disclosure.
    #[test]
    fn catalog_voices_are_pinned_for_local_tts() {
        const MODELS: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../deploy/local/models.sh"
        ));
        // The manifest's lines: repo, revision, file, bytes, SHA-256.
        let pins: Vec<Vec<&str>> = MODELS
            .lines()
            .map(|line| line.split_whitespace().collect::<Vec<_>>())
            .filter(|fields| {
                fields.len() == 5
                    && fields[1].len() == 40
                    && fields[3].parse::<u64>().is_ok()
                    && fields[4].len() == 64
            })
            .collect();
        let kokoro = pins
            .iter()
            .find(|fields| fields[2] == "kokoro-v1_0.safetensors")
            .map(|fields| fields[0])
            .expect("models.sh pins the Kokoro weights");
        for voice in vak_domain::catalog::VOICES {
            let file = format!("voices/{}.safetensors", voice.id);
            assert!(
                pins.iter()
                    .any(|fields| fields[0] == kokoro && fields[2] == file),
                "deploy/local/models.sh doesn't pin {kokoro}/{file}: add it (Hugging Face \
                 revision, size and SHA-256), or local calls with {} fail",
                voice.name
            );
        }
    }

    #[test]
    fn a_missing_model_turns_voice_off_and_bad_settings_fail() {
        assert!(engines(&models(&ALL[..4])).expect("valid").is_none());
        let mut bad_json = ALL.to_vec();
        bad_json[6] = ("VAK_LLM_EXTRA_BODY", "{not json");
        let Err(error) = engines(&models(&bad_json)) else {
            panic!("an extra body that is not JSON is refused")
        };
        assert!(format!("{error:#}").contains("VAK_LLM_EXTRA_BODY"));
        let mut tls = ALL.to_vec();
        tls[2] = ("VAK_STT_BASE_URL", "https://workspace.cloud.databricks.com");
        assert!(engines(&models(&tls)).is_err());
    }
}
