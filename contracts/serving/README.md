# contracts/serving/

Request and response bodies of our speech **Databricks custom model serving endpoints**. Production serves them from MLflow pyfuncs on Model Serving (M7, `deploy/databricks/serving/`); local development serves the same bodies from `vak-model-shim` (`backend/apps/model-shim`), which forwards to mlx-audio. The serving adapters (`vak-adapter-stt-serving`, `vak-adapter-tts-serving`) speak only these shapes, so local and production differ in base URL and auth only.

| File | Endpoint | Model (prod / local) |
|---|---|---|
| `stt.json` | `POST /serving-endpoints/{name}/invocations`, local name `stt` | Parakeet TDT 0.6B v2 (NeMo pyfunc / MLX) |
| `tts.json` | `POST /serving-endpoints/{name}/invocations`, local name `tts` | Kokoro-82M (pyfunc / MLX) |

Each file is a JSON Schema (draft 2020-12) with `$defs/request`, `$defs/response` and `$defs/error`. The contract tests of `vak-model-shim` (feature `model-shim` in `../contracts.toml`) validate every request and response against them, and the signed tag `contract/model-shim/v<N>` pins both files: to change a schema, bump that feature's version first.

## Envelope

The Databricks custom-model envelope [score-custom]: a records-oriented DataFrame in, `predictions` out.

```json
{ "dataframe_records": [ { "...": "one record per utterance or clause, 1 to 8" } ] }
{ "predictions":       [ { "...": "one result per record, same order" } ] }
```

- Send `Content-Type: application/json` and, against Databricks, `Authorization: Bearer <token>`. The local shim ignores auth.
- A request carries 1 to 8 records. The voice path sends one; batching is for offline work. The body limit is 16 MB [serving-limits].
- No streaming: one POST per utterance (STT) or per clause (TTS), as research 01 §5 and 07 §B4 plan.

## Audio encoding

Audio is always **base64 (RFC 4648 §4, standard alphabet, padded) of raw PCM, signed 16-bit little-endian, mono**: no WAV header. Each record says `encoding` and `sample_rate` explicitly, so a later version can add rates or μ-law without guessing.

| | `encoding` | `sample_rate` | Limits |
|---|---|---|---|
| STT input (`audio_b64`) | `pcm_s16le` | 16000 | 1 sample to 30 s (1,280,000 base64 characters); even byte count |
| TTS output (`audio_b64`) | `pcm_s16le` | 24000 (Kokoro's native rate) | at least one sample |

The caller resamples: phone audio (8 kHz μ-law) is decoded and upsampled to 16 kHz before STT, and TTS output is resampled for the transport. A JSON Schema can't say "an even number of bytes", so the shim checks it and answers 400.

## STT

| Request field | Type | |
|---|---|---|
| `audio_b64` | string | required, see above |
| `encoding` | `"pcm_s16le"` | required |
| `sample_rate` | `16000` | required |
| `language` | ISO 639-1, e.g. `"en"` | optional hint; Parakeet v2 is English-only |

Each prediction is `{"text": "<transcript, trimmed>"}`. The text is empty when nothing was recognised.

## TTS

| Request field | Type | |
|---|---|---|
| `text` | string, 1 to 1000 characters, not only whitespace | required |
| `encoding` | `"pcm_s16le"` | required: the format wanted back |
| `sample_rate` | `24000` | required: the rate wanted back |
| `voice` | Kokoro voice ID, e.g. `af_heart` | optional, default `af_heart`. The first letter is the language (`a` US English, `b` UK English) |
| `speed` | 0.5 to 2.0 | optional, default 1.0 |

Each prediction is `{"audio_b64": "...", "encoding": "pcm_s16le", "sample_rate": 24000}`.

## Errors

Every non-200 answer has the Databricks REST error body `{"error_code": "...", "message": "..."}`. The message never echoes text or audio.

| Status | `error_code` | When | Retry? |
|---|---|---|---|
| 400 | `BAD_REQUEST` | the body is not JSON or breaks the schema, bad base64, odd PCM byte count | no: a bug in the caller |
| 404 | `RESOURCE_DOES_NOT_EXIST` | unknown endpoint name | no: configuration |
| 413 | `BAD_REQUEST` | the body is over 16 MiB | no |
| 429 | `REQUEST_LIMIT_EXCEEDED` | Databricks rate limit (never from the shim) | yes, with backoff, within the turn's budget |
| 500 | `INTERNAL_ERROR` | the model failed or answered something unusable (the shim never relabels audio it didn't ask for) | no |
| 503 | `TEMPORARILY_UNAVAILABLE` | the model server can't be reached (scale-from-zero, mlx-audio down) | yes, within the budget |
| 504 | `DEADLINE_EXCEEDED` | the model didn't answer in time | yes, within the budget |

Databricks itself may also answer 400 `BAD_REQUEST` when a pyfunc raises; adapters treat any 4xx other than 429 as not retryable.

## Example

```sh
# STT through the local shim: 1 s of silence (16,000 zero samples = 32,000 bytes).
audio=$(head -c 32000 /dev/zero | base64)
curl -s localhost:8001/serving-endpoints/stt/invocations -H 'content-type: application/json' \
  -d "{\"dataframe_records\":[{\"audio_b64\":\"$audio\",\"encoding\":\"pcm_s16le\",\"sample_rate\":16000}]}"
# {"predictions":[{"text":""}]}
```

## Sources

- [score-custom] Databricks, "Query serving endpoints for custom models": `dataframe_split`, `dataframe_records`, `instances`, `inputs`; responses wrapped in `predictions`. https://docs.databricks.com/aws/en/machine-learning/model-serving/score-custom-model-endpoints
- [serving-limits] Databricks, "Model Serving limits and regions": 16 MB payload. https://docs.databricks.com/aws/en/machine-learning/model-serving/model-serving-limits
- Research: `docs/research/07-databricks-platform.md` §B2–B4 and §D2, `docs/research/01-voice-pipeline-providers.md` §5.
