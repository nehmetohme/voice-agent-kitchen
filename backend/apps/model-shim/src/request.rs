//! Parses and validates invocation bodies against contracts/serving/{stt,tts}.json, with
//! messages that name the field but never echo its value.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Map, Value};

use crate::error::ServingError;

/// Records per request (`maxItems`).
const MAX_RECORDS: usize = 8;
/// STT audio: 30 s of 16 kHz PCM s16le is 960,000 bytes, 1,280,000 base64 characters.
const MAX_STT_AUDIO_B64: usize = 1_280_000;
/// STT input rate.
pub(crate) const STT_SAMPLE_RATE: u32 = 16_000;
/// TTS output rate (Kokoro's native rate).
pub(crate) const TTS_SAMPLE_RATE: u32 = 24_000;
/// The only `encoding` in v1.
pub(crate) const PCM_S16LE: &str = "pcm_s16le";
const MAX_TEXT_CHARS: usize = 1000;
const DEFAULT_VOICE: &str = "af_heart";
const SPEEDS: std::ops::RangeInclusive<f64> = 0.5..=2.0;

/// One utterance to transcribe.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SttRecord {
    /// PCM s16le mono at 16 kHz; never empty, even length.
    pub(crate) pcm: Vec<u8>,
    pub(crate) language: Option<String>,
}

/// One clause to speak, with defaults applied.
#[derive(Debug, PartialEq)]
pub(crate) struct TtsRecord {
    pub(crate) text: String,
    pub(crate) voice: String,
    pub(crate) speed: f64,
}

impl TtsRecord {
    /// Kokoro's pipeline language: the voice's first letter (`a` US English, `b` UK...).
    pub(crate) fn lang_code(&self) -> &str {
        self.voice.get(..1).unwrap_or("a")
    }
}

pub(crate) fn stt_records(body: &[u8]) -> Result<Vec<SttRecord>, ServingError> {
    records(body)?.into_iter().map(stt_record).collect()
}

pub(crate) fn tts_records(body: &[u8]) -> Result<Vec<TtsRecord>, ServingError> {
    records(body)?.into_iter().map(tts_record).collect()
}

/// One STT record: reads every field, rejects unknown ones, then checks the audio.
fn stt_record(mut fields: Fields) -> Result<SttRecord, ServingError> {
    let audio = fields.required_string("audio_b64")?;
    fields.require("encoding", &Value::from(PCM_S16LE), "must be \"pcm_s16le\"")?;
    fields.require(
        "sample_rate",
        &Value::from(STT_SAMPLE_RATE),
        "must be 16000",
    )?;
    let language = fields.optional_string("language")?;
    fields.finish()?;

    if language
        .as_deref()
        .is_some_and(|l| l.len() != 2 || !l.bytes().all(|b| b.is_ascii_lowercase()))
    {
        return Err(fields.invalid("language", "must be an ISO 639-1 code, e.g. \"en\""));
    }
    if audio.len() > MAX_STT_AUDIO_B64 {
        return Err(fields.invalid("audio_b64", "is longer than 30 s"));
    }
    let pcm = BASE64
        .decode(&audio)
        .map_err(|_| fields.invalid("audio_b64", "is not standard padded base64"))?;
    if pcm.is_empty() || pcm.len() % 2 != 0 {
        return Err(fields.invalid(
            "audio_b64",
            "must hold whole 16-bit samples (an even, non-zero byte count)",
        ));
    }
    Ok(SttRecord { pcm, language })
}

/// One TTS record: reads every field, rejects unknown ones, applies the defaults, then
/// checks the values.
fn tts_record(mut fields: Fields) -> Result<TtsRecord, ServingError> {
    let text = fields.required_string("text")?;
    fields.require("encoding", &Value::from(PCM_S16LE), "must be \"pcm_s16le\"")?;
    fields.require(
        "sample_rate",
        &Value::from(TTS_SAMPLE_RATE),
        "must be 24000",
    )?;
    let voice = fields
        .optional_string("voice")?
        .unwrap_or_else(|| DEFAULT_VOICE.to_owned());
    let speed = fields.optional_number("speed")?.unwrap_or(1.0);
    fields.finish()?;

    if text.trim().is_empty() || text.chars().count() > MAX_TEXT_CHARS {
        return Err(fields.invalid(
            "text",
            "must have 1 to 1000 characters, not only whitespace",
        ));
    }
    if !is_kokoro_voice(&voice) {
        return Err(fields.invalid("voice", "must be a Kokoro voice ID, e.g. \"af_heart\""));
    }
    if !SPEEDS.contains(&speed) {
        return Err(fields.invalid("speed", "must be from 0.5 to 2.0"));
    }
    Ok(TtsRecord { text, voice, speed })
}

/// `^[a-z][fm]_[a-z]+$`
fn is_kokoro_voice(voice: &str) -> bool {
    let bytes = voice.as_bytes();
    bytes.len() > 3
        && bytes[0].is_ascii_lowercase()
        && matches!(bytes[1], b'f' | b'm')
        && bytes[2] == b'_'
        && bytes[3..].iter().all(u8::is_ascii_lowercase)
}

/// The `dataframe_records` of a body, 1 to 8 objects, and nothing else at the top.
fn records(body: &[u8]) -> Result<Vec<Fields>, ServingError> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|_| ServingError::bad_request("the body is not valid JSON"))?;
    let Value::Object(mut top) = value else {
        return Err(ServingError::bad_request("the body must be a JSON object"));
    };
    let records = top.remove("dataframe_records").ok_or_else(|| {
        ServingError::bad_request("the body must have `dataframe_records` (and nothing else)")
    })?;
    if !top.is_empty() {
        return Err(ServingError::bad_request(
            "the body must have only `dataframe_records`",
        ));
    }
    let Value::Array(records) = records else {
        return Err(ServingError::bad_request(
            "`dataframe_records` must be an array",
        ));
    };
    if records.is_empty() || records.len() > MAX_RECORDS {
        return Err(ServingError::bad_request(
            "`dataframe_records` must have 1 to 8 records",
        ));
    }
    records
        .into_iter()
        .enumerate()
        .map(|(index, record)| match record {
            Value::Object(map) => Ok(Fields { index, map }),
            _ => Err(ServingError::bad_request(format!(
                "dataframe_records[{index}] must be an object"
            ))),
        })
        .collect()
}

/// The fields of one record; each accessor removes what it reads, so `finish` can reject
/// the rest. `null` counts as absent.
struct Fields {
    index: usize,
    map: Map<String, Value>,
}

impl Fields {
    fn invalid(&self, key: &str, problem: &str) -> ServingError {
        ServingError::bad_request(format!("dataframe_records[{}].{key} {problem}", self.index))
    }

    fn take(&mut self, key: &str) -> Option<Value> {
        self.map.remove(key).filter(|value| !value.is_null())
    }

    fn optional_string(&mut self, key: &str) -> Result<Option<String>, ServingError> {
        match self.take(key) {
            None => Ok(None),
            Some(Value::String(value)) => Ok(Some(value)),
            Some(_) => Err(self.invalid(key, "must be a string")),
        }
    }

    fn required_string(&mut self, key: &str) -> Result<String, ServingError> {
        self.optional_string(key)?
            .ok_or_else(|| self.invalid(key, "is required"))
    }

    fn optional_number(&mut self, key: &str) -> Result<Option<f64>, ServingError> {
        match self.take(key) {
            None => Ok(None),
            Some(Value::Number(value)) => Ok(value.as_f64()),
            Some(_) => Err(self.invalid(key, "must be a number")),
        }
    }

    /// A required field that must equal `expected`.
    fn require(&mut self, key: &str, expected: &Value, problem: &str) -> Result<(), ServingError> {
        match self.take(key) {
            Some(value) if value == *expected => Ok(()),
            _ => Err(self.invalid(key, problem)),
        }
    }

    fn finish(&self) -> Result<(), ServingError> {
        match self.map.keys().next() {
            None => Ok(()),
            Some(key) => Err(self.invalid(key, "is not a field of this contract")),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn body(records: &Value) -> Vec<u8> {
        json!({ "dataframe_records": records })
            .to_string()
            .into_bytes()
    }

    fn stt(record: &Value) -> Result<Vec<SttRecord>, String> {
        stt_records(&body(&json!([record]))).map_err(|e| format!("{e:?}"))
    }

    fn tts(record: &Value) -> Result<Vec<TtsRecord>, String> {
        tts_records(&body(&json!([record]))).map_err(|e| format!("{e:?}"))
    }

    #[test]
    fn stt_records_decode_their_pcm_and_keep_order() {
        let records = stt_records(&body(&json!([
            { "audio_b64": "AQACAA==", "encoding": "pcm_s16le", "sample_rate": 16000 },
            { "audio_b64": "AwA=", "encoding": "pcm_s16le", "sample_rate": 16000, "language": "en" },
        ])))
        .expect("valid");

        assert_eq!(
            records,
            [
                SttRecord {
                    pcm: vec![1, 0, 2, 0],
                    language: None
                },
                SttRecord {
                    pcm: vec![3, 0],
                    language: Some("en".to_owned())
                },
            ]
        );
    }

    #[test]
    fn tts_records_get_defaults_and_a_lang_code() {
        let records = tts_records(&body(&json!([
            { "text": "Hi.", "encoding": "pcm_s16le", "sample_rate": 24000 },
            { "text": "Bye.", "encoding": "pcm_s16le", "sample_rate": 24000,
              "voice": "bm_george", "speed": 1.5 },
        ])))
        .expect("valid");

        assert_eq!(records[0].voice, "af_heart");
        assert!((records[0].speed - 1.0).abs() < f64::EPSILON);
        assert_eq!(records[0].lang_code(), "a");
        assert_eq!(records[1].voice, "bm_george");
        assert!((records[1].speed - 1.5).abs() < f64::EPSILON);
        assert_eq!(records[1].lang_code(), "b");
    }

    #[test]
    fn invalid_envelopes_are_rejected() {
        for body in [
            b"nope".as_slice(),
            b"[]",
            b"{}",
            br#"{"dataframe_records": [], "x": 1}"#,
            br#"{"dataframe_records": {}}"#,
            br#"{"dataframe_records": []}"#,
            br#"{"dataframe_records": [1]}"#,
        ] {
            assert!(
                stt_records(body).is_err(),
                "{}",
                String::from_utf8_lossy(body)
            );
        }
        let nine = vec![json!({ "text": "a", "encoding": "pcm_s16le", "sample_rate": 24000 }); 9];
        assert!(tts_records(&body(&json!(nine))).is_err());
    }

    #[test]
    fn invalid_stt_fields_are_rejected_without_echoing_values() {
        let ok = json!({ "audio_b64": "AAA=", "encoding": "pcm_s16le", "sample_rate": 16000 });
        let with = |key: &str, value: Value| {
            let mut record = ok.clone();
            record[key] = value;
            record
        };
        assert!(stt(&ok).is_ok());
        for (record, field) in [
            (with("audio_b64", json!(null)), "audio_b64 is required"),
            (with("audio_b64", json!(7)), "audio_b64 must be a string"),
            (
                with("audio_b64", json!("SECRET!!")),
                "audio_b64 is not standard",
            ),
            (
                with("audio_b64", json!("AAAA")),
                "audio_b64 must hold whole",
            ),
            (
                with("audio_b64", json!("A".repeat(1_280_004))),
                "audio_b64 is longer",
            ),
            (with("encoding", json!("wav")), "encoding must be"),
            (
                with("sample_rate", json!(8000)),
                "sample_rate must be 16000",
            ),
            (with("language", json!("eng")), "language must be"),
            (with("language", json!("EN")), "language must be"),
            (with("SECRET", json!(1)), "SECRET is not a field"),
        ] {
            let error = stt(&record).expect_err(field);
            assert!(error.contains(field), "{error} should mention {field}");
            assert!(!error.contains("SECRET!!"), "{error}");
        }
    }

    #[test]
    fn invalid_tts_fields_are_rejected() {
        let ok = json!({ "text": "Hi.", "encoding": "pcm_s16le", "sample_rate": 24000 });
        let with = |key: &str, value: Value| {
            let mut record = ok.clone();
            record[key] = value;
            record
        };
        assert!(tts(&ok).is_ok());
        assert!(tts(&with("text", json!("é".repeat(1000)))).is_ok());
        for (record, field) in [
            (with("text", json!("   ")), "text must have"),
            (with("text", json!("é".repeat(1001))), "text must have"),
            (with("text", json!(false)), "text must be a string"),
            (
                with("sample_rate", json!(16000)),
                "sample_rate must be 24000",
            ),
            (with("voice", json!("Heart")), "voice must be"),
            (with("voice", json!("ax_heart")), "voice must be"),
            (with("voice", json!("af_")), "voice must be"),
            (with("speed", json!(0.49)), "speed must be from"),
            (with("speed", json!("fast")), "speed must be a number"),
        ] {
            let error = tts(&record).expect_err(field);
            assert!(error.contains(field), "{error} should mention {field}");
        }
    }
}
