//! Contract `model-shim` v1: the dev-only `vak-model-shim` serves the Databricks custom-model
//! invocation shapes of `contracts/serving/{stt,tts}.json` and forwards them to mlx-audio.
//!
//! Black-box: every test starts the real binary against a wiremock stand-in for mlx-audio,
//! configured only through these environment variables:
//!
//! | Variable | Meaning |
//! |---|---|
//! | `VAK_SHIM_ADDR` | listen address, `IP:port` |
//! | `VAK_SHIM_MLX_AUDIO_URL` | mlx-audio base URL |
//! | `VAK_SHIM_STT_MODEL`, `VAK_SHIM_TTS_MODEL` | the `model` sent to mlx-audio |
//! | `VAK_SHIM_TIMEOUT_MS` | deadline of each forwarded call |
//! | `VAK_LOG` | log filter |
//!
//! What it pins:
//! - `GET /health` answers 200 once the shim is ready.
//! - `POST /serving-endpoints/stt/invocations`: each record becomes one multipart
//!   `POST /v1/audio/transcriptions` (`file` = the PCM as a 16 kHz mono 16-bit WAV, `model`,
//!   `response_format=json`, `language` when given); predictions keep record order and carry
//!   the trimmed text.
//! - `POST /serving-endpoints/tts/invocations`: each record becomes one JSON
//!   `POST /v1/audio/speech` (`model`, `input`, `voice` default `af_heart`, `speed` default 1.0,
//!   `lang_code` = the voice's first letter, `response_format=wav`); predictions carry the WAV's
//!   samples as base64 PCM. Audio in any other format is an error, never relabelled.
//! - Errors use the Databricks shape `{error_code, message}`: 400 `BAD_REQUEST` for invalid
//!   input (nothing is forwarded), 404 `RESOURCE_DOES_NOT_EXIST` for another endpoint name,
//!   500 `INTERNAL_ERROR` when mlx-audio fails, 503 `TEMPORARILY_UNAVAILABLE` when it is
//!   unreachable, 504 `DEADLINE_EXCEEDED` when it is too slow.
//! - Every request a test sends and every response it gets is checked against the schemas.
//! - Text and audio never reach the logs, even at `trace`.
//!
//! Contract file: never edit it once it is tagged `contract/model-shim/v1`. To change the
//! contract, add a new version.

// Clippy allows these only inside #[test] fns; here the helpers belong to the tests too.
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers: a failed expectation is a test failure"
)]

use std::collections::HashMap;
use std::net::TcpListener;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Value, json};
use tokio::io::AsyncReadExt as _;
use tokio::process::{Child, Command};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const STT_MODEL: &str = "test/parakeet";
const TTS_MODEL: &str = "test/kokoro";

// ---- Schemas -------------------------------------------------------------------------------

/// A validator for `contracts/serving/<file>`'s `$defs/<def>`.
fn schema(file: &str, def: &str) -> jsonschema::Validator {
    let path = format!(
        "{}/../../../contracts/serving/{file}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"));
    let mut root: Value = serde_json::from_str(&text).expect("the schema is JSON");
    root["$ref"] = json!(format!("#/$defs/{def}"));
    jsonschema::validator_for(&root).unwrap_or_else(|e| panic!("{file} is a valid schema: {e}"))
}

fn assert_valid(file: &str, def: &str, instance: &Value) {
    let validator = schema(file, def);
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "{file} $defs/{def} rejects {instance:.200}: {errors:?}"
    );
}

fn assert_invalid(file: &str, def: &str, instance: &Value) {
    assert!(
        !schema(file, def).is_valid(instance),
        "{file} $defs/{def} should reject {instance:.200}"
    );
}

// ---- Audio helpers -------------------------------------------------------------------------

fn pcm(samples: &[i16]) -> Vec<u8> {
    samples.iter().flat_map(|s| s.to_le_bytes()).collect()
}

fn ramp(len: usize) -> Vec<i16> {
    (0..len)
        .map(|i| i16::try_from(i % 2000).expect("fits") - 1000)
        .collect()
}

/// A canonical 44-byte-header PCM16 WAV.
fn wav(sample_rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
    let data = pcm(samples);
    let data_len = u32::try_from(data.len()).expect("small");
    let block_align = channels * 2;
    let mut out = Vec::with_capacity(44 + data.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * u32::from(block_align)).to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(&data);
    out
}

struct Wav {
    format: u16,
    channels: u16,
    sample_rate: u32,
    bits: u16,
    data: Vec<u8>,
}

/// Parses a RIFF/WAVE file by walking its chunks.
fn parse_wav(bytes: &[u8]) -> Wav {
    assert!(bytes.len() >= 12, "WAV too short");
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    let u32_at =
        |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let (mut fmt, mut data) = (None, None);
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = u32_at(at + 4) as usize;
        let body = at + 8;
        match id {
            b"fmt " => {
                fmt = Some((
                    u16_at(body),
                    u16_at(body + 2),
                    u32_at(body + 4),
                    u16_at(body + 14),
                ));
            }
            b"data" => data = Some(bytes[body..body + len].to_vec()),
            _ => {}
        }
        at = body + len + len % 2;
    }
    let (format, channels, sample_rate, bits) = fmt.expect("a fmt chunk");
    Wav {
        format,
        channels,
        sample_rate,
        bits,
        data: data.expect("a data chunk"),
    }
}

// ---- Fake mlx-audio ------------------------------------------------------------------------

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// The fields of a `multipart/form-data` body, by name.
fn multipart_fields(request: &Request) -> HashMap<String, Vec<u8>> {
    let content_type = request
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .expect("a content-type");
    assert!(
        content_type.starts_with("multipart/form-data"),
        "{content_type}"
    );
    let boundary = content_type
        .split("boundary=")
        .nth(1)
        .expect("a boundary")
        .trim_matches('"');
    let delimiter = format!("--{boundary}");
    let mut fields = HashMap::new();
    let mut rest = request.body.as_slice();
    let start = find(rest, delimiter.as_bytes()).expect("a first delimiter");
    rest = &rest[start + delimiter.len()..];
    while !rest.starts_with(b"--") {
        let part_end = find(rest, delimiter.as_bytes()).expect("a closing delimiter");
        let part = rest[..part_end]
            .strip_prefix(b"\r\n")
            .expect("CRLF after delimiter");
        let header_end = find(part, b"\r\n\r\n").expect("part headers");
        let headers = std::str::from_utf8(&part[..header_end]).expect("ASCII headers");
        let name = headers
            .split("name=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("a field name");
        let content = &part[header_end + 4..];
        let content = content
            .strip_suffix(b"\r\n")
            .expect("CRLF before delimiter");
        fields.insert(name.to_owned(), content.to_vec());
        rest = &rest[part_end + delimiter.len()..];
    }
    fields
}

fn text_field(fields: &HashMap<String, Vec<u8>>, name: &str) -> Option<String> {
    fields
        .get(name)
        .map(|v| String::from_utf8(v.clone()).expect("UTF-8 field"))
}

/// Fake STT: "transcribes" the uploaded WAV as its sample count and rate, padded with spaces.
struct FakeStt;

impl Respond for FakeStt {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let fields = multipart_fields(request);
        let audio = parse_wav(fields.get("file").expect("a file field"));
        let samples = audio.data.len() / 2;
        ResponseTemplate::new(200).set_body_json(
            json!({ "text": format!("  {samples} samples at {} Hz \n", audio.sample_rate) }),
        )
    }
}

/// Fake TTS: answers a 24 kHz mono WAV of `10 × input length` ramp samples.
struct FakeTts {
    sample_rate: u32,
}

impl Respond for FakeTts {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).expect("a JSON body");
        let input = body["input"].as_str().expect("an input string");
        let samples = ramp(input.chars().count() * 10);
        ResponseTemplate::new(200)
            .insert_header("content-type", "audio/wav")
            .set_body_bytes(wav(self.sample_rate, 1, &samples))
    }
}

async fn fake_mlx_audio() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(FakeStt)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/speech"))
        .respond_with(FakeTts {
            sample_rate: 24_000,
        })
        .mount(&server)
        .await;
    server
}

/// A mock server answering every request with `template`.
async fn upstream_answering(template: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(template)
        .mount(&server)
        .await;
    server
}

// ---- The shim under test -------------------------------------------------------------------

struct Shim {
    base: String,
    logs: Arc<Mutex<String>>,
    /// Killed when the test drops it.
    child: Child,
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("a free port")
        .port()
}

impl Shim {
    async fn start(mlx_audio_url: &str) -> Self {
        Self::start_with(mlx_audio_url, &[]).await
    }

    async fn start_with(mlx_audio_url: &str, env: &[(&str, &str)]) -> Self {
        let addr = format!("127.0.0.1:{}", free_port());
        let mut command = Command::new(env!("CARGO_BIN_EXE_vak-model-shim"));
        command
            .env_clear()
            .env("VAK_SHIM_ADDR", &addr)
            .env("VAK_SHIM_MLX_AUDIO_URL", mlx_audio_url)
            .env("VAK_SHIM_STT_MODEL", STT_MODEL)
            .env("VAK_SHIM_TTS_MODEL", TTS_MODEL)
            .env("VAK_SHIM_TIMEOUT_MS", "5000")
            .env("VAK_LOG", "info,vak_model_shim=trace")
            .envs(env.iter().copied())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().expect("the shim binary starts");

        let logs = Arc::new(Mutex::new(String::new()));
        let mut stderr = child.stderr.take().expect("stderr is piped");
        let sink = Arc::clone(&logs);
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            while let Ok(n) = stderr.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                sink.lock()
                    .expect("log lock")
                    .push_str(&String::from_utf8_lossy(&buf[..n]));
            }
        });

        let mut shim = Self {
            base: format!("http://{addr}"),
            logs,
            child,
        };
        shim.wait_ready().await;
        shim
    }

    async fn wait_ready(&mut self) {
        let client = reqwest::Client::new();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(response) = client.get(format!("{}/health", self.base)).send().await {
                assert_eq!(response.status(), 200, "GET /health");
                return;
            }
            if let Ok(Some(exit)) = self.child.try_wait() {
                panic!(
                    "the shim exited before it was ready ({exit}); logs:\n{}",
                    self.logs()
                );
            }
            assert!(
                Instant::now() < deadline,
                "the shim was not ready within 10 s; logs:\n{}",
                self.logs()
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    fn logs(&self) -> String {
        self.logs.lock().expect("log lock").clone()
    }

    /// POSTs `body` to `/serving-endpoints/<endpoint>/invocations`; returns status and JSON.
    async fn invoke(&self, endpoint: &str, body: impl Into<reqwest::Body>) -> (u16, Value) {
        let response = reqwest::Client::new()
            .post(format!(
                "{}/serving-endpoints/{endpoint}/invocations",
                self.base
            ))
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .expect("the shim answers");
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        assert_eq!(
            content_type.as_deref(),
            Some("application/json"),
            "status {status}"
        );
        let body = response.json().await.expect("a JSON body");
        (status, body)
    }

    async fn invoke_json(&self, endpoint: &str, body: &Value) -> (u16, Value) {
        self.invoke(endpoint, body.to_string()).await
    }
}

fn assert_error(file: &str, status: u16, body: &Value, want_status: u16, want_code: &str) {
    assert_eq!(status, want_status, "{body}");
    assert_valid(file, "error", body);
    assert_eq!(body["error_code"], want_code, "{body}");
}

fn stt_record(samples: &[i16]) -> Value {
    json!({
        "audio_b64": BASE64.encode(pcm(samples)),
        "encoding": "pcm_s16le",
        "sample_rate": 16000,
    })
}

fn tts_record(text: &str) -> Value {
    json!({ "text": text, "encoding": "pcm_s16le", "sample_rate": 24000 })
}

// ---- Tests ---------------------------------------------------------------------------------

#[tokio::test]
async fn health_answers_200() {
    let upstream = fake_mlx_audio().await;
    let shim = Shim::start(&upstream.uri()).await;

    let response = reqwest::get(format!("{}/health", shim.base))
        .await
        .expect("the shim answers");

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn stt_forwards_each_record_as_a_wav_upload_and_keeps_record_order() {
    let upstream = fake_mlx_audio().await;
    let shim = Shim::start(&upstream.uri()).await;
    let first = ramp(1600);
    let second = ramp(800);
    let mut with_language = stt_record(&second);
    with_language["language"] = json!("en");
    let request = json!({ "dataframe_records": [stt_record(&first), with_language] });
    assert_valid("stt.json", "request", &request);

    let (status, body) = shim.invoke_json("stt", &request).await;

    assert_eq!(status, 200, "{body}");
    assert_valid("stt.json", "response", &body);
    assert_eq!(
        body,
        json!({ "predictions": [
            { "text": "1600 samples at 16000 Hz" },
            { "text": "800 samples at 16000 Hz" },
        ]})
    );
    let received = upstream.received_requests().await.expect("recording is on");
    assert_eq!(received.len(), 2);
    for (request, samples, language) in [
        (&received[0], &first, None),
        (&received[1], &second, Some("en")),
    ] {
        assert_eq!(request.url.path(), "/v1/audio/transcriptions");
        let fields = multipart_fields(request);
        assert_eq!(text_field(&fields, "model").as_deref(), Some(STT_MODEL));
        assert_eq!(
            text_field(&fields, "response_format").as_deref(),
            Some("json")
        );
        assert_eq!(text_field(&fields, "language").as_deref(), language);
        let audio = parse_wav(fields.get("file").expect("a file field"));
        assert_eq!(
            (audio.format, audio.channels, audio.sample_rate, audio.bits),
            (1, 1, 16_000, 16)
        );
        assert_eq!(audio.data, pcm(samples));
    }
}

#[tokio::test]
async fn tts_forwards_each_record_as_speech_json_and_returns_base64_pcm() {
    let upstream = fake_mlx_audio().await;
    let shim = Shim::start(&upstream.uri()).await;
    let mut custom = tts_record("Bye.");
    custom["voice"] = json!("bm_george");
    custom["speed"] = json!(1.25);
    let request = json!({ "dataframe_records": [tts_record("Hello there."), custom] });
    assert_valid("tts.json", "request", &request);

    let (status, body) = shim.invoke_json("tts", &request).await;

    assert_eq!(status, 200, "{body}");
    assert_valid("tts.json", "response", &body);
    let predictions = body["predictions"].as_array().expect("predictions");
    assert_eq!(predictions.len(), 2);
    for (prediction, text) in predictions.iter().zip(["Hello there.", "Bye."]) {
        assert_eq!(prediction["encoding"], "pcm_s16le");
        assert_eq!(prediction["sample_rate"], 24000);
        let audio = BASE64
            .decode(prediction["audio_b64"].as_str().expect("audio_b64"))
            .expect("standard base64");
        assert_eq!(audio, pcm(&ramp(text.chars().count() * 10)), "{text}");
    }
    let received = upstream.received_requests().await.expect("recording is on");
    assert_eq!(received.len(), 2);
    let sent: Vec<Value> = received
        .iter()
        .map(|r| {
            assert_eq!(r.url.path(), "/v1/audio/speech");
            serde_json::from_slice(&r.body).expect("a JSON body")
        })
        .collect();
    let field = |i: usize, name: &str| sent[i][name].clone();
    assert_eq!(field(0, "model"), TTS_MODEL);
    assert_eq!(field(0, "input"), "Hello there.");
    assert_eq!(field(0, "voice"), "af_heart");
    assert_eq!(field(0, "speed").as_f64(), Some(1.0));
    assert_eq!(field(0, "lang_code"), "a");
    assert_eq!(field(0, "response_format"), "wav");
    assert_eq!(field(1, "input"), "Bye.");
    assert_eq!(field(1, "voice"), "bm_george");
    assert_eq!(field(1, "speed").as_f64(), Some(1.25));
    assert_eq!(field(1, "lang_code"), "b");
}

#[tokio::test]
async fn tts_audio_in_another_format_is_an_internal_error_not_relabelled() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/speech"))
        .respond_with(FakeTts {
            sample_rate: 22_050,
        })
        .mount(&upstream)
        .await;
    let shim = Shim::start(&upstream.uri()).await;

    let (status, body) = shim
        .invoke_json("tts", &json!({ "dataframe_records": [tts_record("Hi.")] }))
        .await;

    assert_error("tts.json", status, &body, 500, "INTERNAL_ERROR");
}

#[tokio::test]
async fn invalid_requests_are_400_bad_request_and_nothing_is_forwarded() {
    let upstream = fake_mlx_audio().await;
    let shim = Shim::start(&upstream.uri()).await;
    let audio = stt_record(&ramp(160));
    let with = |record: &Value, key: &str, value: Value| {
        let mut record = record.clone();
        record[key] = value;
        json!({ "dataframe_records": [record] })
    };
    let without = |record: &Value, key: &str| {
        let mut record = record.clone();
        record.as_object_mut().expect("an object").remove(key);
        json!({ "dataframe_records": [record] })
    };
    let clause = tts_record("Hello.");
    let schema_invalid = [
        ("stt", "stt.json", json!({})),
        ("stt", "stt.json", json!({ "dataframe_records": [] })),
        ("stt", "stt.json", json!({ "inputs": [audio.clone()] })),
        (
            "stt",
            "stt.json",
            json!({ "dataframe_records": vec![audio.clone(); 9] }),
        ),
        ("stt", "stt.json", without(&audio, "audio_b64")),
        (
            "stt",
            "stt.json",
            with(&audio, "audio_b64", json!("not base64!")),
        ),
        ("stt", "stt.json", with(&audio, "sample_rate", json!(8000))),
        ("stt", "stt.json", with(&audio, "encoding", json!("wav"))),
        (
            "stt",
            "stt.json",
            with(&audio, "language", json!("english")),
        ),
        ("stt", "stt.json", with(&audio, "extra", json!(true))),
        ("tts", "tts.json", json!({ "dataframe_records": "Hello." })),
        ("tts", "tts.json", with(&clause, "text", json!(""))),
        ("tts", "tts.json", with(&clause, "text", json!("   "))),
        (
            "tts",
            "tts.json",
            with(&clause, "text", json!("a".repeat(1001))),
        ),
        ("tts", "tts.json", with(&clause, "voice", json!("Heart"))),
        ("tts", "tts.json", with(&clause, "speed", json!(3.0))),
        (
            "tts",
            "tts.json",
            with(&clause, "sample_rate", json!(16000)),
        ),
        ("tts", "tts.json", without(&clause, "encoding")),
    ];
    for (endpoint, file, request) in &schema_invalid {
        assert_invalid(file, "request", request);
        let (status, body) = shim.invoke_json(endpoint, request).await;
        assert_error(file, status, &body, 400, "BAD_REQUEST");
    }

    // Not JSON at all, and PCM with an odd byte count (which a schema can't express).
    let (status, body) = shim.invoke("stt", "hello").await;
    assert_error("stt.json", status, &body, 400, "BAD_REQUEST");
    let odd = with(&audio, "audio_b64", json!(BASE64.encode([0u8, 1, 2])));
    assert_valid("stt.json", "request", &odd);
    let (status, body) = shim.invoke_json("stt", &odd).await;
    assert_error("stt.json", status, &body, 400, "BAD_REQUEST");

    let received = upstream.received_requests().await.expect("recording is on");
    assert!(received.is_empty(), "forwarded {} requests", received.len());
}

#[tokio::test]
async fn another_endpoint_name_is_404_resource_does_not_exist() {
    let upstream = fake_mlx_audio().await;
    let shim = Shim::start(&upstream.uri()).await;

    let (status, body) = shim
        .invoke_json("llm", &json!({ "dataframe_records": [tts_record("Hi.")] }))
        .await;

    assert_error("tts.json", status, &body, 404, "RESOURCE_DOES_NOT_EXIST");
}

#[tokio::test]
async fn failing_mlx_audio_is_500_internal_error() {
    let upstream = upstream_answering(ResponseTemplate::new(500).set_body_string("boom")).await;
    let shim = Shim::start(&upstream.uri()).await;

    let (status, body) = shim
        .invoke_json(
            "stt",
            &json!({ "dataframe_records": [stt_record(&ramp(160))] }),
        )
        .await;
    assert_error("stt.json", status, &body, 500, "INTERNAL_ERROR");

    let (status, body) = shim
        .invoke_json("tts", &json!({ "dataframe_records": [tts_record("Hi.")] }))
        .await;
    assert_error("tts.json", status, &body, 500, "INTERNAL_ERROR");
}

#[tokio::test]
async fn malformed_mlx_audio_answers_are_500_internal_error() {
    let upstream = upstream_answering(ResponseTemplate::new(200).set_body_string("<html>")).await;
    let shim = Shim::start(&upstream.uri()).await;

    let (status, body) = shim
        .invoke_json(
            "stt",
            &json!({ "dataframe_records": [stt_record(&ramp(160))] }),
        )
        .await;
    assert_error("stt.json", status, &body, 500, "INTERNAL_ERROR");

    let (status, body) = shim
        .invoke_json("tts", &json!({ "dataframe_records": [tts_record("Hi.")] }))
        .await;
    assert_error("tts.json", status, &body, 500, "INTERNAL_ERROR");
}

#[tokio::test]
async fn unreachable_mlx_audio_is_503_temporarily_unavailable() {
    let closed = format!("http://127.0.0.1:{}", free_port());
    let shim = Shim::start(&closed).await;

    let (status, body) = shim
        .invoke_json(
            "stt",
            &json!({ "dataframe_records": [stt_record(&ramp(160))] }),
        )
        .await;
    assert_error("stt.json", status, &body, 503, "TEMPORARILY_UNAVAILABLE");

    let (status, body) = shim
        .invoke_json("tts", &json!({ "dataframe_records": [tts_record("Hi.")] }))
        .await;
    assert_error("tts.json", status, &body, 503, "TEMPORARILY_UNAVAILABLE");
}

#[tokio::test]
async fn slow_mlx_audio_is_504_deadline_exceeded_within_the_timeout() {
    let slow = ResponseTemplate::new(200)
        .set_body_json(json!({ "text": "late" }))
        .set_delay(Duration::from_secs(10));
    let upstream = upstream_answering(slow).await;
    let shim = Shim::start_with(&upstream.uri(), &[("VAK_SHIM_TIMEOUT_MS", "300")]).await;

    let started = Instant::now();
    let (status, body) = shim
        .invoke_json(
            "stt",
            &json!({ "dataframe_records": [stt_record(&ramp(160))] }),
        )
        .await;

    assert_error("stt.json", status, &body, 504, "DEADLINE_EXCEEDED");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "answered after {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn text_and_audio_never_reach_the_logs() {
    const SECRET: &str = "Purple elephants dance at midnight";
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "text": SECRET })))
        .mount(&upstream)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/speech"))
        .respond_with(FakeTts {
            sample_rate: 24_000,
        })
        .mount(&upstream)
        .await;
    let shim = Shim::start(&upstream.uri()).await;
    let audio = stt_record(&ramp(4000));
    let audio_b64 = audio["audio_b64"].as_str().expect("a string").to_owned();

    let (stt_status, stt_body) = shim
        .invoke_json("stt", &json!({ "dataframe_records": [audio] }))
        .await;
    let (tts_status, tts_body) = shim
        .invoke_json("tts", &json!({ "dataframe_records": [tts_record(SECRET)] }))
        .await;
    let (bad_status, _) = shim
        .invoke_json(
            "tts",
            &json!({ "dataframe_records": [{ "text": SECRET, "speed": 9 }] }),
        )
        .await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!((stt_status, tts_status, bad_status), (200, 200, 400));
    assert_eq!(stt_body["predictions"][0]["text"], SECRET);
    let tts_audio = tts_body["predictions"][0]["audio_b64"]
        .as_str()
        .expect("audio_b64")
        .to_owned();
    let logs = shim.logs();
    assert!(
        logs.contains("stt") && logs.contains("tts"),
        "requests are logged:\n{logs}"
    );
    for word in ["Purple", "elephants", "midnight"] {
        assert!(!logs.contains(word), "logs contain {word:?}:\n{logs}");
    }
    assert!(
        !logs.contains(&audio_b64[..24]),
        "logs contain request audio"
    );
    assert!(
        !logs.contains(&tts_audio[..24]),
        "logs contain response audio"
    );
}
