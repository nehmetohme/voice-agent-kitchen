//! Where the voice models are: the LLM, STT, TTS and decision endpoints. Local development
//! and Databricks differ only in these values (base URL, model or endpoint, credentials).

use secrecy::SecretString;

use crate::ConfigError;

/// Sets [`LlmSettings::base_url`]: the OpenAI-compatible API root, e.g. `http://host:8080/v1`.
pub const LLM_BASE_URL_VAR: &str = "VAK_LLM_BASE_URL";
/// Sets [`LlmSettings::model`]. Required with [`LLM_BASE_URL_VAR`].
pub const LLM_MODEL_VAR: &str = "VAK_LLM_MODEL";
/// Sets [`LlmSettings::api_key`]: a bearer token, if the server wants one. A secret.
pub const LLM_API_KEY_VAR: &str = "VAK_LLM_API_KEY";
/// Sets [`LlmSettings::extra_body`]: a JSON object merged into every chat request.
pub const LLM_EXTRA_BODY_VAR: &str = "VAK_LLM_EXTRA_BODY";
/// Sets the STT [`ServingSettings::base_url`]: the serving root (or the local model shim).
pub const STT_BASE_URL_VAR: &str = "VAK_STT_BASE_URL";
/// Sets the STT [`ServingSettings::endpoint`]. Default `stt`.
pub const STT_ENDPOINT_VAR: &str = "VAK_STT_ENDPOINT";
/// Sets the TTS [`ServingSettings::base_url`].
pub const TTS_BASE_URL_VAR: &str = "VAK_TTS_BASE_URL";
/// Sets the TTS [`ServingSettings::endpoint`]. Default `tts`.
pub const TTS_ENDPOINT_VAR: &str = "VAK_TTS_ENDPOINT";
/// Sets [`DecisionSettings::base_url`]: the System One server root.
pub const DECISION_BASE_URL_VAR: &str = "VAK_DECISION_BASE_URL";
/// Sets [`DecisionSettings::model`]. Required with [`DECISION_BASE_URL_VAR`].
pub const DECISION_MODEL_VAR: &str = "VAK_DECISION_MODEL";
/// Sets [`DecisionSettings::path`]. Default `/v1/systemone` (llama-server); Databricks
/// serves `/ai-gateway/typesafe/v1/systemone`.
pub const DECISION_PATH_VAR: &str = "VAK_DECISION_PATH";

/// The voice models. Voice sessions run only when all four are set.
#[derive(Clone, Debug, Default)]
pub struct ModelSettings {
    pub llm: Option<LlmSettings>,
    pub stt: Option<ServingSettings>,
    pub tts: Option<ServingSettings>,
    pub decisions: Option<DecisionSettings>,
}

/// An OpenAI-compatible chat-completions endpoint.
#[derive(Clone, Debug)]
pub struct LlmSettings {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<SecretString>,
    /// JSON text; the composition root parses it.
    pub extra_body: Option<String>,
}

/// A Databricks-shaped serving endpoint (`/serving-endpoints/{endpoint}/invocations`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServingSettings {
    pub base_url: String,
    pub endpoint: String,
}

/// A System One decision endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionSettings {
    pub base_url: String,
    pub model: String,
    pub path: Option<String>,
}

impl ModelSettings {
    /// Whether every model is set, so voice sessions can run.
    #[must_use]
    pub const fn complete(&self) -> bool {
        self.llm.is_some() && self.stt.is_some() && self.tts.is_some() && self.decisions.is_some()
    }

    /// The first model variable left unset, for a startup notice.
    #[must_use]
    pub const fn first_missing(&self) -> Option<&'static str> {
        if self.llm.is_none() {
            Some(LLM_BASE_URL_VAR)
        } else if self.stt.is_none() {
            Some(STT_BASE_URL_VAR)
        } else if self.tts.is_none() {
            Some(TTS_BASE_URL_VAR)
        } else if self.decisions.is_none() {
            Some(DECISION_BASE_URL_VAR)
        } else {
            None
        }
    }

    pub(crate) fn from_values(
        value: &impl Fn(&str) -> Option<String>,
    ) -> Result<Self, ConfigError> {
        let required = |var: &'static str| value(var).ok_or(ConfigError::Missing { var });
        let llm = value(LLM_BASE_URL_VAR)
            .map(|base_url| -> Result<LlmSettings, ConfigError> {
                Ok(LlmSettings {
                    base_url,
                    model: required(LLM_MODEL_VAR)?,
                    api_key: value(LLM_API_KEY_VAR).map(SecretString::from),
                    extra_body: value(LLM_EXTRA_BODY_VAR),
                })
            })
            .transpose()?;
        let serving = |base: &str, endpoint: &str, default: &str| {
            value(base).map(|base_url| ServingSettings {
                base_url,
                endpoint: value(endpoint).unwrap_or_else(|| default.to_owned()),
            })
        };
        let decisions = value(DECISION_BASE_URL_VAR)
            .map(|base_url| -> Result<DecisionSettings, ConfigError> {
                Ok(DecisionSettings {
                    base_url,
                    model: required(DECISION_MODEL_VAR)?,
                    path: value(DECISION_PATH_VAR),
                })
            })
            .transpose()?;
        Ok(Self {
            llm,
            stt: serving(STT_BASE_URL_VAR, STT_ENDPOINT_VAR, "stt"),
            tts: serving(TTS_BASE_URL_VAR, TTS_ENDPOINT_VAR, "tts"),
            decisions,
        })
    }
}

#[cfg(test)]
mod tests {
    use secrecy::ExposeSecret;

    use crate::Settings;

    fn lookup<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    const ALL: [(&str, &str); 7] = [
        ("VAK_LLM_BASE_URL", "http://gpu:8080/v1"),
        ("VAK_LLM_MODEL", "KAT-Coder"),
        ("VAK_LLM_API_KEY", "sk-hunter2"),
        (
            "VAK_LLM_EXTRA_BODY",
            r#"{"chat_template_kwargs":{"enable_thinking":false}}"#,
        ),
        ("VAK_STT_BASE_URL", "http://127.0.0.1:8001"),
        ("VAK_TTS_BASE_URL", "http://127.0.0.1:8001"),
        ("VAK_DECISION_BASE_URL", "http://127.0.0.1:8081"),
    ];

    #[test]
    fn no_models_by_default_and_voice_needs_all_four() {
        let none = Settings::from_lookup(lookup(&[])).expect("valid");
        assert!(!none.models.complete());
        assert_eq!(none.models.first_missing(), Some("VAK_LLM_BASE_URL"));

        let mut vars = ALL.to_vec();
        vars.push(("VAK_DECISION_MODEL", "Kev-4B"));
        let all = Settings::from_lookup(lookup(&vars)).expect("valid");
        assert!(all.models.complete());
        assert_eq!(all.models.first_missing(), None);
        let llm = all.models.llm.as_ref().expect("set");
        assert_eq!(llm.model, "KAT-Coder");
        assert_eq!(
            llm.api_key.as_ref().map(ExposeSecret::expose_secret),
            Some("sk-hunter2")
        );
        assert!(
            llm.extra_body
                .as_deref()
                .is_some_and(|body| body.contains("thinking"))
        );
        assert_eq!(
            all.models.stt.as_ref().map(|stt| stt.endpoint.as_str()),
            Some("stt")
        );
        assert_eq!(
            all.models.tts.as_ref().map(|tts| tts.endpoint.as_str()),
            Some("tts")
        );
        let decisions = all.models.decisions.as_ref().expect("set");
        assert_eq!(decisions.model, "Kev-4B");
        assert_eq!(decisions.path, None);
        assert!(
            !format!("{all:?}").contains("hunter2"),
            "the API key is a secret"
        );

        let partial =
            Settings::from_lookup(lookup(&[("VAK_STT_BASE_URL", "http://x")])).expect("valid");
        assert!(!partial.models.complete());
        assert_eq!(partial.models.first_missing(), Some("VAK_LLM_BASE_URL"));
    }

    #[test]
    fn a_base_url_needs_its_model() {
        for (vars, missing) in [
            (vec![("VAK_LLM_BASE_URL", "http://gpu/v1")], "VAK_LLM_MODEL"),
            (
                vec![("VAK_DECISION_BASE_URL", "http://127.0.0.1:8081")],
                "VAK_DECISION_MODEL",
            ),
        ] {
            let error = Settings::from_lookup(lookup(&vars)).expect_err("incomplete");
            assert_eq!(error.to_string(), format!("{missing} must be set"));
        }
    }

    #[test]
    fn endpoints_and_the_decision_path_can_be_overridden() {
        let settings = Settings::from_lookup(lookup(&[
            ("VAK_STT_BASE_URL", "http://ws"),
            ("VAK_STT_ENDPOINT", "parakeet"),
            ("VAK_TTS_BASE_URL", "http://ws"),
            ("VAK_TTS_ENDPOINT", "kokoro"),
            ("VAK_DECISION_BASE_URL", "http://ws"),
            ("VAK_DECISION_MODEL", "system.ai.openjev-qwen35-4b"),
            ("VAK_DECISION_PATH", "/ai-gateway/typesafe/v1/systemone"),
        ]))
        .expect("valid");
        let models = settings.models;
        assert_eq!(
            models.stt.map(|stt| stt.endpoint).as_deref(),
            Some("parakeet")
        );
        assert_eq!(
            models.tts.map(|tts| tts.endpoint).as_deref(),
            Some("kokoro")
        );
        assert_eq!(
            models.decisions.and_then(|d| d.path).as_deref(),
            Some("/ai-gateway/typesafe/v1/systemone")
        );
        assert_eq!(models.llm.map(|llm| llm.base_url), None);
    }
}
