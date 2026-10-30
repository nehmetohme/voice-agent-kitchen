# 07: Databricks platform fit (Apps, Model Serving, Lakebase)

> Researched 2026-10-30 against docs.databricks.com (AWS), the 2026 release notes, databricks.com pricing and GitHub. Prices are US list (AWS us-east-1, before discounts). "est." marks my own estimates. "Docs silent" means I looked and found nothing, not "no". Labels like [permissions] point to Sources at the end.
> Scope: Databricks fit only. The carrier and pipeline design are in `01-voice-pipeline-providers.md`; Rust crates are in `02-rust-backend.md`.

## TL;DR

- **Hard blocker: a carrier can't reach a Databricks App.** "You can't make Databricks apps public. Anonymous access and bypassing single sign-on (SSO) are not supported" [permissions].
  - Telephony webhooks, media streams and a public web widget all need a **thin public edge relay outside Databricks**: a small Rust service in the same AWS region.
  - The relay forwards to the app's `/api/*` routes with a service-principal OAuth token [connect-local].
  - **"Entirely on Databricks" is therefore not achievable as written. This needs your approval.**
- **A Rust binary on Apps is feasible, but not out of the box.**
  - Apps build only Python and Node.js, run Ubuntu 22.04, and reject any single file over 10 MB [app-development].
  - Ship the binary as a **Python wheel stored in a Unity Catalog (UC) Volume** and installed through `requirements.txt` [dependencies].
  - WebSockets work in practice, but they're undocumented. A ~120 s proxy timeout is reported for HTTP [community-504]. **Long-lived call WebSockets must be proven in a spike first.**
- **Model Serving has no native streaming speech-to-text (STT) and no text-to-speech (TTS) at all.** It is request/response only, with server-sent event (SSE) output streaming for chat [score-custom], [agent-stream]. Self-host on warm A10G GPUs:
  - STT: NVIDIA **Parakeet TDT 0.6B** as an MLflow pyfunc, or **Whisper-large-v3-turbo** through the documented custom-LLM (vLLM) `audio/transcriptions` task [custom-llm].
  - TTS: **Kokoro-82M** as a pyfunc.
  - The pipeline becomes: voice-activity detection (VAD) segments each utterance, one STT POST per utterance, the LLM streams over SSE, and TTS runs per sentence.
- **The LLM on the Foundation Model APIs (FMAPI) is fine.** It's OpenAI-compatible, streams over SSE, and supports tools. Claude Haiku 5.5, GPT-OSS-20B/120B and Llama 3.1 8B/3.3 70B are available pay-per-token in us-east-1/-2 and us-west-2 [fm-models], [model-regions].
  - There's **no published time to first token (TTFT) and no SLA** [fm-limits].
- **Latency estimate (phone): P50 ≈ 1.1–1.3 s, P95 ≈ 2–2.5 s.** That misses the ~1 s goal. Getting under 1 s needs everything to go right: aggressive end-of-turn, a short first clause, and a fast LLM.
  - **Cost:** about $2.5–5.3k/month fixed for warm GPUs and Apps, plus about $0.014/min variable.
  - At 100k min/month that's about **$0.04–0.07/min**, against ~$0.047 for the vendor-API stack in doc 01.
- **Lakebase is a good fit (GA).**
  - Postgres 16–18, pgvector, copy-on-write **branches per PR**, scale-to-zero, about $50/month for a 1 capacity unit (CU) always-on compute.
  - The OAuth token used as the password lasts 1 h and is checked only at login.
  - sqlx has no password callback, so refresh it with `Pool::set_connect_options` [lb-auth], [sqlx-3117].

## Feasibility verdicts

| Component | Verdict | Why (details in sections A–D) |
|---|---|---|
| Rust binary on Databricks Apps | 🟡 | No Rust runtime and a 10 MB file limit. Use a wheel from a UC Volume. CPU architecture is undocumented (spike S1) |
| Browser ↔ App WebSocket (operator test calls) | 🟡 | Works in practice (Streamlit, Labs app); undocumented. Timeouts unknown (S2) |
| **Carrier/widget → App ingress** | 🔴 → 🟢 with relay | No anonymous access. A public edge relay plus a service-principal bearer token on `/api/*` fixes it |
| App egress to carrier APIs | 🟢 | Open by default. Stable egress IPs and FQDN policies are available [networking] |
| Identity, resources, on-behalf-of-user (OBO), SSO for the dashboard | 🟢 | One service principal per app. Lakebase and serving endpoints bind as resources [resources], [auth] |
| Deploy: bundles + GitHub OIDC, dev/stg/prod | 🟢 | Official recipe [cicd]. Bundles cover apps, endpoints and Postgres [bundle-app] |
| Observability | 🟢 | App telemetry GA: OpenTelemetry → UC tables [observability] |
| LLM on FMAPI / Unity Gateway | 🟡 | Works. No TTFT SLA; Haiku 5.5 has low output-token rate limits [fm-limits] |
| STT on Model Serving | 🟡 | No native streaming. Self-hosted batch per utterance on a warm A10G (S3) |
| TTS on Model Serving | 🟡 (highest risk) | No native TTS, no documented audio streaming. Sentence-level calls (S4) |
| Warm GPU cost | 🟡 | About $1,022/month per A10G at 24×7. Scale-to-zero cold starts make it unusable for production [endpoints] |
| Lakebase from Rust (sqlx) | 🟢 | GA. Token refresh workaround needed. The pooler doesn't support OAuth [lb-pool] |
| Lakebase → Delta analytics | 🟢/🟡 | Synced tables GA; Lakebase Change Data Feed (CDF) to Delta is Public Preview [lb-cdf] |
| Local-dev parity on macOS | 🟢 | `llama-server`, `mlx-audio` and Postgres in Docker, all behind the same wire contracts |

## Architecture diagrams

**Production.** One region, for example us-east-1. The only non-Databricks compute is the edge relay.

```
 PSTN caller ─► Carrier (Telnyx / Twilio)
                  │ webhooks (HTTPS) + media WebSocket (8 kHz μ-law)
                  ▼
 ┌─ Public edge (AWS, same region) ─────────────────────────────┐
 │ edge-relay (Rust, 2 small tasks behind an ALB)               │
 │  - verifies carrier signatures, owns the carrier media leg   │
 │  - SP OAuth M2M token cache (or workload identity fed.)      │
 │  - later: web-widget token + WebSocket/WebRTC edge           │
 └──────────────┬───────────────────────────────────────────────┘
                │ wss://<app>.databricksapps.com/api/media/{call}
                │ Authorization: Bearer <SP token>, X-Routing-Key: {call}
 Operator ──SSO─┤ (dashboard + browser test calls go direct)
                ▼
 ┌─ Databricks workspace ───────────────────────────────────────────────────┐
 │ Databricks App "vak" (Rust wheel; Large, 2 instances, horizontal scaling)│
 │  routers → services → voice pipeline (Silero VAD + Smart Turn, codecs)   │
 │    │ HTTP/2 keep-alive, OAuth M2M token cache (scoped for route-opt.)    │
 │    ├─► STT endpoint (GPU_MEDIUM A10G, pyfunc Parakeet, route-optimized)  │
 │    ├─► TTS endpoint (GPU_MEDIUM A10G, pyfunc Kokoro, route-optimized)    │
 │    ├─► Unity Gateway / FMAPI LLM, streamed over SSE (Haiku 5.5 | GPT-OSS)│
 │    ├─► Lakebase Postgres (sqlx, direct TLS, OAuth token as password)     │
 │    └─► OpenTelemetry collector (localhost:4314) → UC otel_* tables       │
 │ Lakebase CDF → Delta (call analytics) · UC Volume (app wheels, models)   │
 └──────────────┬───────────────────────────────────────────────────────────┘
                └─► egress via stable NCC IPs → carrier REST API (dial, transfer, hang up)
```

**Fallback if S2 fails ("outbound-dial").**
- The app opens *outbound* WebSockets to the relay; or it joins a LiveKit room as the DIVA blueprint linked from the Databricks blog does [diva], [db-voice-blog].
- The relay then multiplexes calls onto those connections, so media never crosses the Apps ingress proxy.

**Local development (macOS, Apple Silicon).** `just dev` starts `docker compose up db` plus process-compose.

```
 Browser ─WS─► vak-server (cargo run, native)
                 ├─► llama-server :8080  OpenAI chat, gpt-oss-20b GGUF, Metal  (same weights as databricks-gpt-oss-20b)
                 ├─► shim :8001 /serving-endpoints/{stt,tts}/invocations ─► mlx_audio.server :8000 (Parakeet/Whisper, Kokoro; MLX)
                 └─► Postgres 17 + pgvector (Docker)
 Carrier testing: edge-relay (cargo run) + cloudflared/ngrok tunnel, the same code path as prod
```

---

## A. Databricks Apps

### A1. Runtime and running a Rust binary
- **Environment:** Ubuntu 22.04, Python 3.11 (any version through `uv`), Node 22. No CPU architecture or glibc version is documented [system-env].
- **Build flow:**
  1. `npm install` (only if `package.json` exists)
  2. `pip install -r requirements.txt` or `uv sync`
  3. `npm run build`
  4. the `app.yaml` `command` [deploy]
- **`command` doesn't run in a shell.** Only `DATABRICKS_APP_PORT` is substituted, so `command: ['vak-server']` is valid [app-runtime].
- **Restrictions:**
  - No custom container image and no `apt-get`; the app runs as a non-root user.
  - The app must listen on `0.0.0.0:$DATABRICKS_APP_PORT`.
  - The proxy terminates TLS and **requires HTTP/2 cleartext (h2c)** to the app.
  - The app gets 15 s from `SIGTERM` before `SIGKILL` [best-practices].
- **axum settings:** enable the `http2` feature, and route WebSockets with `any(ws)` instead of `get(ws)` so WebSocket over HTTP/2 works [axum].
- **"App files can't exceed 10 MB"** per file [app-development]. A 30–80 MB binary doesn't fit as source. No total-size limit is documented.
- **Delivery options, ranked:**
  1. **Wheel in a UC Volume.** maturin `bindings="bin"` produces the wheel. Reference its absolute `/Volumes/...` path in `requirements.txt`, and attach the volume as an app resource [dependencies], [uc-volumes]. The wheel filename is the version, which gives rollback for free.
  2. The same wheel in a private index: `PIP_INDEX_URL` from a secret, or the workspace default repository (GA 2026-10-14) [rn-oct].
  3. A `launch.py` that downloads from the Volume into `/tmp` and calls `os.execv`. DIVA installs its worker at boot the same way [diva-deploy].
  4. Split the binary into 9 MB chunks (a hack).
- **Build target:** `x86_64-unknown-linux-musl`, a static binary with rustls and no glibc dependency. Log `uname -m` at startup (S1).

### A2. Compute, scaling, price
| Size | vCPU / RAM | DBU/h | $/h at $0.75/DBU | 24×7 per month |
|---|---|---|---|---|
| Medium (default) | 2 / 6 GB | 0.5 | $0.38 | ~$270 |
| Large | 4 / 12 GB | 1 | $0.75 | ~$540 |
| XLarge | 12 / 48 GB | 3 | $2.25 | ~$1,620 |

Sources: [compute-size], [apps-pricing].

- **No GPU.** Apps are CPU-only; inference belongs in Model Serving [best-practices]. In-app CPU work (Silero VAD, Smart Turn, resampling) is cheap.
- **Horizontal scaling (GA 2026-09)** [horizontal-scaling], [rn-sep]:
  - 1–5 instances (raisable on request), at most 20 such apps per workspace.
  - **The instance count is fixed; autoscaling isn't documented.** Bundles expose `compute_min/max_instances`, but there are no semantics docs for it [bundle-app].
  - Rolling zero-downtime deploys.
  - Best-effort affinity by cookie or by the **`X-Routing-Key`** header.
- 100 apps per workspace [limits]. Billing runs only while the app is running [key-concepts].

### A3. WebSockets, streaming, timeouts
- **Apps docs never mention WebSockets.** Evidence they work:
  - Streamlit, a flagship Apps framework, runs over a WebSocket.
  - Databricks Labs' coding-agents app ships Socket.IO WebSocket terminal I/O on Apps [labs-ws].
- **Timeouts:**
  - Community answers report a fixed **~120 s ingress timeout** for HTTP (504 `upstream request timeout`) [community-504], and a stream that froze at 120 s [community-120s].
  - **No WebSocket idle or maximum-lifetime limit is documented.**
- Clients connect over TLS 1.3 [networking]. The proxy talks h2c to the app [best-practices].
- **Redeploy drops calls.** Old instances get `SIGTERM` and have 15 s before `SIGKILL`. Mitigations:
  - The relay keeps the carrier leg up and reconnects to a healthy instance.
  - Deploy outside calling windows.

### A4. Authentication and ingress (critical)
- **No anonymous access.**
  - Apps can't be public; anonymous access and SSO bypass aren't supported [permissions].
  - Users must belong to the Databricks account. External users are onboarded through SCIM/JIT [key-concepts].
  - An iframe embed still needs a Databricks login [embed].
- **Machine access** is OAuth bearer tokens on **`/api/` routes only**, for callers with `CAN USE` [connect-local].
- **Carriers can't supply that token:**
  - **Twilio** webhooks and `<Stream>` can't send a rotating Databricks bearer token. Custom data arrives in the first message's `customParameters` [twilio-ws].
  - **Telnyx** has `stream_auth_token` ("sent as part of the WebSocket connection"). The docs don't say which header, so it might carry a per-call token for the media leg (worth testing) [telnyx-stream].
  - Both carriers' call-control webhooks are unauthenticated public POSTs anyway.
  - **⇒ Direct carrier ingress is 🔴.**
- **Workaround: the edge relay** (`crates/edge-relay`, about 300–500 lines of code; est. $20–60/month on ECS Fargate or App Runner with an ALB).
  - **Responsibilities:**
    1. Verify carrier signatures: Twilio HMAC, Telnyx Ed25519.
    2. Accept the media WebSocket.
    3. Dial the app's `/api/media/{call}` with a **dedicated service principal's** token. Use OAuth M2M, or workload identity federation from AWS so there's no secret [wif].
    4. Set `X-Routing-Key` and pump frames in both directions.
    5. Reconnect if the app drops.
    6. Rate-limit and filter spam.
  - **What it implies:**
    - One more hop, about 1–5 ms within the region (est.).
    - One more service to patch and monitor.
    - Audio PII crosses it: use TLS, and never log payloads.
    - Branded public hostnames live on the relay. Apps have no custom domain; the URL is fixed as `<app>-<ws-id>.<region>.databricksapps.com` [key-concepts]. The 2026-07 "Custom URLs" preview is for the account login page only [rn-jul].
  - **IP access lists** can restrict the app to the relay's IPs [networking].
- **Operators get SSO for free.**
  - The proxy forwards `X-Forwarded-Email`/`-User` [http-headers], plus `x-forwarded-access-token` when OBO scopes are set [auth].
  - The browser test-call WebSocket goes straight to the app with the SSO cookie.

### A5. Egress
- **Open by default.** DIVA reaches LiveKit, Deepgram and ElevenLabs from an app [diva-deploy].
- **Controls:**
  - Network Connectivity Configurations (NCC) give stable egress IPs, which carriers can allowlist.
  - Network policies (Enterprise tier) allow FQDN allowlists, with dry-run mode and the `system.access.outbound_network` denial log [networking].
  - Allow the carrier APIs, the relay, and `pypi.org` (build time).
- **UDP egress is undocumented.** That only matters for WebRTC; use TURN over TLS.

### A6. Identity, environment variables, resources, OBO
- **Service principal:** each app has its own, and it can't be changed. Credentials arrive in env vars [auth], [system-env]:
  - `DATABRICKS_CLIENT_ID/SECRET`, `DATABRICKS_HOST`, `DATABRICKS_APP_PORT`, `DATABRICKS_APP_NAME`, `DATABRICKS_WORKSPACE_ID`
  - `OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4314` when telemetry is on.
  - Normalise `DATABRICKS_HOST`: the docs don't say whether it includes `https://`.
- **Resources** bind at deploy time and are read through `valueFrom` [resources], [env-vars]:
  - serving endpoint (`CAN QUERY`)
  - Unity Gateway model service (`Can execute`)
  - Lakebase (`postgres` key)
  - secret, Volume, another app
- **OBO:** the user's token arrives in `x-forwarded-access-token`, limited to the declared scopes (`model-serving`, `postgres`, `sql`, …) [auth].
  - **Voice path:** use the service principal, since calls have no user.
  - **Dashboard:** use OBO only for governed UC analytics reads.

### A7. Deployment, environments, monitoring
- **Bundles:** Databricks Asset Bundles were **renamed "Declarative Automation Bundles"** on 2026-03-16 [rn-mar]. A bundle can declare `apps` (`compute_size`, `git_source`, `user_api_scopes`, telemetry), `model_serving_endpoint`, `postgres_project/branch/endpoint/role`, `volume` and `secret_scope`. One `databricks.yml` per environment target [bundle-app].
- **GitHub Actions** [cicd]:
  - OIDC with `DATABRICKS_AUTH_TYPE=github-oidc`, so no stored secret.
  - Steps: `bundle validate` → `bundle deploy` → **`bundle run <app>`**. `deploy` alone doesn't restart the app.
  - Then poll `apps get` until the state is `RUNNING`.
  - Git-backed deploys are GA [rn-apr].
- **Environments:** Databricks recommends **separate workspaces** for dev, staging and prod [best-practices].
  - **Rollback:** redeploy a previous commit or wheel; there's no rollback button [deploy].
- **Monitoring:**
  - stdout/stderr appear in the Logs tab, which shows one instance at a time.
  - **App telemetry (GA 2026-09)** writes OTel logs, spans and metrics to UC Delta tables [observability]. Point Rust `opentelemetry-otlp` (gRPC) at the env var and per-turn latency spans become SQL-queryable.

## B. Model Serving for real-time voice

### B1. LLMs: FMAPI and Unity Gateway
- **Modes:**
  - Pay-per-token, and **priority** pay-per-token (`service_tier:"priority"`, no TTFT guarantee; **not offered for Claude or Llama**) [priority].
  - Provisioned throughput, on-demand or reserved, for Llama, GPT-OSS, Gemma and Qwen [fmapi].
- **Models now** [fm-models]:
  - **Claude:** Haiku 5.5/4.5, Sonnet 5.5/5/4.6/4.5, Opus 5.5…4.1.
  - **Llama:** 3.1 8B, 3.3 70B, 4 Maverick.
  - **GPT-OSS:** 20B/120B.
  - **Gemini:** 3.x Flash/Pro and 4 Argon. Most need cross-geo routing.
  - Qwen3.5-122B (preview), Qwen3-Next-80B, GPT-5/6, Grok, GLM, Kimi, DeepSeek, Gemma 3.
  - us-east-1/-2 and us-west-2 have Claude, Llama and GPT-OSS without cross-geo [model-regions].
- **APIs:**
  - OpenAI-compatible: `https://<ws>/ai-gateway/mlflow/v1/chat/completions` (with `model=system.ai.<name>`), `/responses`, `/embeddings` [ug-query].
  - The legacy `/serving-endpoints/chat/completions` with `model=<endpoint>` still works [fm-api-ref].
  - Native: `/ai-gateway/anthropic/v1/messages` [anthropic-messages].
  - SSE streaming [fm-api-ref].
- **Tool calling** [function-calling]:
  - Supported for Claude Sonnet/Opus, Llama 3.x, GPT-OSS and Gemini. Max 32 tools; no parallel calls.
  - **Haiku 5.5 tool use is documented only through the Anthropic Messages API.**
  - Claude 5.5 models reject `response_format` on Chat Completions [structured-outputs].
  - Reasoning models can return `content` as a **list** [reason-models]. Use tolerant serde types; async-openai's `byot` feature handles this.
- **Limits** (Enterprise pay-per-token) [fm-limits]:
  - Most models allow 1M input tokens/min and 100K output tokens/min.
  - **Haiku 5.5 allows only 200K input / 20K output tokens per minute.** Because `max_tokens` is reserved up front, `max_tokens=150` gives about 130 turns/min, roughly **20 concurrent calls per workspace** (est.). Ask for a quota increase, or use GPT-OSS at 100K/min.
  - `max_tokens` is pre-reserved against the output quota, so keep it small.
  - Platform overhead "<50 ms"; 503 means capacity, so retry or fall back.
- **Unity Gateway (GA 2026-08)** [ug], [ug-traffic], [ug-inference-tables], [service-policies]:
  - Rate limits, budgets, usage system tables, traffic split and fallback on 429/5xx, inference tables (asynchronous).
  - Guardrails ("service policies", Beta) run LLM judges in parallel, which adds latency. **Keep them off the hot path.**
  - Custom endpoints (our STT/TTS) **aren't** gateway destinations [ug-services].
- **Price** at $0.07/DBU [pfm-pricing], [fm-pricing]:

  | Model | Input / output, $ per M tokens |
  |---|---|
  | Haiku 5.5 (short context) | ≈ $0.10 / $0.50 |
  | GPT-OSS-20B | ≈ $0.07 / $0.30 |
  | Llama 3.1 8B | ≈ $0.15 / $0.45 |

- **Recommendation:** benchmark Haiku 5.5 (reasoning off, Messages API) against GPT-OSS-120B/20B and Llama 3.1 8B (Chat Completions) in S5.
  - GPT-OSS-20B runs locally with identical weights, which gives the best parity.
  - Configure a Unity Gateway fallback across models.

### B2. STT: no Databricks-native streaming STT
- **What Databricks has:**
  - **`ai_transcribe`** (Beta 2026-08-29): SQL/Python batch only, English and Spanish, no REST [ai-transcribe]. Useful for post-call re-transcription, not for live turns.
  - **Custom LLM Serving** (Public Preview; launched 2026-05-20) supports the task `llm/v1/audio/transcriptions` [custom-llm]:
    - OpenAI-shaped, multipart `/serving-endpoints/<n>/audio/transcriptions`.
    - A **Whisper-large-v3-turbo on A10G** starter notebook [whisper-nb].
    - **Limits:** no route optimization, no dynamic autoscaling, scale-from-zero takes "one to several minutes", US regions only.
  - **Gemini audio input** (`audio_url` data URI) on FMAPI [audio-models]. A fused STT+LLM turn is possible, but it's cross-geo and its latency is unknown. Experiment only.
- **Recommended custom models**, from the Open ASR leaderboard 2026-10-28 (wideband English, *not* 8 kHz) [asr-board]:

  | Model | WER | License | Serving path | Notes |
  |---|---|---|---|---|
  | **Parakeet TDT 0.6B v2** | 6.0% | CC-BY-4.0 | NeMo pyfunc on GPU_MEDIUM, **route-optimized** | Very fast. **Default for English** [parakeet] |
  | Parakeet v3 | 6.2% | CC-BY-4.0 | NeMo pyfunc | 25 EU languages |
  | Whisper-large-v3-turbo | 7.6% | MIT | vLLM `audio/transcriptions` | Multilingual; documented path; padded 30 s encoder means more latency |
  | Qwen3-ASR-1.7B | 5.4% | Apache-2.0 | vLLM (support unverified) | |

  - Canary-Qwen 2.5B, Granite Speech and Voxtral also rank well.
  - Kyutai streaming STT needs its own WebSocket server, so it doesn't fit Model Serving [kyutai].

### B3. TTS: no Databricks-native TTS
- No TTS in FMAPI or the release notes, and custom LLM serving has no `audio/speech` task [custom-llm], [fm-models].
- TTS must therefore be a classic **MLflow pyfunc on a GPU endpoint** returning base64 PCM.

| Model | License | Quality / latency (vendor claims) | Fit |
|---|---|---|---|
| **Kokoro-82M** | Apache-2.0 | Good; ~300 ms first chunk on GPU in Kokoro-FastAPI, likely faster for one short sentence | **Default.** Tiny; a T4 may suffice [kokoro], [kokoro-fastapi] |
| Orpheus 3B | Apache-2.0 card, Llama-3.2 base (licence unclear) | Expressive; ~200 ms when streamed, but we can't stream | Needs an L40S (est.) [orpheus] |
| Chatterbox-Turbo 350M | MIT | Low latency; **watermarks every output** | Alternative [chatterbox] |
| Kyutai TTS 1.6B | CC-BY-4.0 | Streams text in | Its WebSocket server doesn't fit Model Serving [kyutai] |
| Qwen3-TTS, VibeVoice-0.5B, Dia2 | Apache / MIT | Newer streaming models | Evaluate later |
| Piper | **GPL-3.0** now | CPU-fast, robotic | Avoid (licence) [piper] |
| F5-TTS, XTTS-v2, Voxtral-TTS, Higgs TTS 3 | Non-commercial | — | **Excluded** |

### B4. Streaming capabilities and what they mean for the pipeline
- **Input is JSON** (`inputs` / `dataframe_split`) or a protobuf KServe v2 tensor (Public Preview, numeric types only) [score-custom], [rn-jul].
- **Output streaming is documented only for chat and agents** (`predict_stream`, `ResponsesAgent`) [agent-stream].
- **No streaming input, no bidirectional streaming, no WebSockets.**
- **Payload limit:** 16 MB; timeout 597 s [serving-limits], [serving-timeouts]. Three seconds of 16 kHz PCM16 is about 96 KB.
- **Consequences for the pipeline:**
  1. **VAD and turn detection run in the Rust app** (Silero + Smart Turn on ONNX, CPU). There's no STT-native end-of-turn like Deepgram Flux, so budget about 300 ms more than doc 01.
  2. **STT:** one POST per utterance. Optionally, "pseudo-stream" by sending the rolling utterance every ~0.5–1 s, so partial transcripts feed barge-in checks and speculative LLM starts. The final pass runs at end-of-turn. Parakeet makes this cheap.
  3. **LLM:** SSE, cut at the first clause.
  4. **TTS:** one POST per sentence or clause. Keep the first one to ≤ 8 words. Run the calls in parallel with ordered playback.
  5. Request 8 kHz output for phone, or resample in Rust.
  6. Barge-in cancels in-flight HTTP requests.

### B5. GPU serving
- **Workload types** [custom-models], [custom-llm]:

  | Type | GPU | Notes |
  |---|---|---|
  | GPU_SMALL | T4 | |
  | **GPU_MEDIUM** | **A10G** | |
  | MULTIGPU_MEDIUM | 4×A10G | |
  | GPU_LARGE (Beta) | L40S | us-east-1/-2, us-west-2, eu-central-1, ap-northeast-1/-2 |
  | GPU_XLARGE | H100 | us-west-2, enrollment only |
  | GPU_LARGE_RTX | RTX PRO 6000 | |

- **Scaling** [endpoints]:
  - Replicas = provisioned concurrency ÷ 4. Scale-up is near-immediate; scale-down happens every 5 min.
  - **Scale-to-zero** after 30 min idle, with cold starts of "usually 10–20 s, can take minutes", and no SLA. **Disable it in production**, which Databricks also advises.
  - GPU capacity is not guaranteed when acquiring new replicas.
  - Periodic maintenance reloads happen, so run N+1 replicas.
- **Route optimization** (custom models only; set at creation) [route-opt], [route-opt-query]:
  - Overhead **<20 ms** [serving-limits].
  - A dedicated hostname: `https://<id>.serving.cloud.databricks.com/<ws-id>/serving-endpoints/<n>/invocations`.
  - **OAuth only**, with tokens scoped by `authorization_details`.
- **Limits:** 200 QPS per workspace normally, 300K route-optimized.
- **Cost:** GPU DBU/h [ms-pricing] × $0.07/DBU [fm-pricing]:

  | GPU | $/h | Per month 24×7 |
  |---|---|---|
  | T4 | $0.73 | ~$536 |
  | **A10G** | **$1.40** | **~$1,022** |
  | L40S | $3.14 | ~$2,292 |

  - **STT + TTS warm on one A10G each ≈ $2,044/month. With N+1 ≈ $4,088.**
  - Capacity estimate (unmeasured): roughly 30–60 concurrent calls per A10G pair. S3/S4 will measure it.

### B6. Network: Apps → serving
- No latency figure is documented for Apps to serving endpoints in the same region. Apps sit on the same serverless plane as route-optimized serving [networking].
- Documented overheads: <20 ms route-optimized and <50 ms FMAPI [serving-limits], [fm-limits].
- **Estimate:** 5–30 ms per call with HTTP/2 keep-alive and cached tokens. Measure p50/p99 in S3.

## C. Lakebase

### C1. Offering, status, features, price
- **Single platform.** Lakebase Provisioned instances were migrated to **Lakebase Autoscaling**; the migration finished 2026-07. New instances have been Autoscaling projects since 2026-03 [lb-upgrade].
- **Status:**
  - GA on AWS 2026-01-22 and Azure 2026-03-02; GCP is Beta.
  - The Postgres REST API, CLI and SDKs are GA as of 2026-08 [lb-rn].
- **Versions:** **Postgres 16, 17 (default) and 18** [lb-rn].
- **Extensions** [lb-ext]:
  - Available: `vector` 0.8.x, `pg_trgm`, `pgcrypto`, `uuid-ossp`, `pg_stat_statements`, PostGIS, `citext`, `pg_jsonschema`, …
  - **Not available:** `pg_cron`, `timescaledb`.
- **Branching:** copy-on-write and "instant" [lb-branches]. Manage with `databricks postgres create-branch … {"source_branch":…,"ttl":"86400s"}`; max TTL 30 days, 500 branches and 20 active computes per project [lb-manage-branches], [lb-projects].
  - **This gives one branch per PR or preview environment, plus migration dry-runs against a copy of prod.**
  - Docs don't cover compute auto-creation, start time, or whether roles are copied to branches. Test this in S7.
- **Scale-to-zero:** configurable from 60 s to 7 days. Resume takes "a few hundred ms", or "a few seconds" according to the Apps page; the two disagree. Suspension drops sessions [lb-s2z].
  - Use it for dev and branches, **off for prod**.
- **Sizing:** 1 CU ≈ 2 GB. 0.5–64 CU autoscaling, up to 112 CU fixed. Max connections: 1 CU → 218, 4 CU → 894 [lb-computes].
- **Price** (AWS Premium) [lb-pricing]:
  - Compute $0.092/CU-h on autoscaling, $0.069 always-on. Storage $0.345/GB-month.
  - A banner advertises 50% off until 2027-01-31; it isn't clear whether the shown prices already include it.
  - **1 CU always-on ≈ $50/month**; 0.5 CU ≈ $25.

### C2. Auth, Apps binding, sqlx, TLS, pooling
- **Tokens** [lb-auth], [lb-manual]:
  - `POST /api/2.0/postgres/credentials {"endpoint":"projects/<p>/branches/<b>/endpoints/<e>"}` returns `{token, expire_time}`.
  - The TTL is **1 h**. **"Token expiration is enforced only at login. Open connections remain active."**
  - Idle connections close after 24 h; connections may be closed after 3 days.
  - The raw flow from Rust: an `/oidc/v1/token` client-credentials token, then the credentials call.
- **Native passwords:** off by default since 2026-05; manual rotation. Needed only for the pooler [lb-roles].
- **Role mapping:** `databricks_create_role('<sp-uuid>','SERVICE_PRINCIPAL')`, run with the `databricks_auth` extension [lb-roles].
- **Apps binding** (Lakebase resource, permission "Can connect and create") [apps-lakebase]:
  - Injects `PGHOST/PGPORT/PGDATABASE/PGUSER/PGSSLMODE/PGAPPNAME`. **No password is injected; the app mints tokens itself.**
  - `valueFrom: postgres` yields the endpoint path [env-vars].
  - The official Python sample mints a token for every new pooled connection [lb-apps-tutorial].
- **sqlx (0.8/0.9):** no `before_connect` hook (issue #3117 is open; the connector rewrite PR is unmerged) [sqlx-3117]. Pattern:
  1. `PgConnectOptions::new()` reads the `PG*` variables.
  2. Mint a token and create the pool with `max_lifetime` 30–50 min, `idle_timeout` ~10 min and `test_before_acquire`.
  3. A tokio task refreshes the token ~10 min before expiry and calls **`pool.set_connect_options(base.password(new))`**, which affects new connections only [sqlx-pool].
  4. Retry once after a scale-to-zero resume.
- **TLS:** `sslmode=require` is the only documented mode. `verify-full` and the CA are undocumented, so test with rustls and webpki roots [lb-conn].
- **Pooler:** the built-in PgBouncer runs in transaction mode and supports **password roles only, no OAuth**. It also doesn't support prepared statements or advisory locks. **Connect directly** and size the sqlx pool under the compute's `max_connections` [lb-pool].
- No Apps→Lakebase latency figure is documented. The same region implies low single-digit ms (est.).

### C3. Migrations and analytics
- **CI migrations** [gh-oidc]:
  1. GitHub OIDC → `databricks postgres generate-database-credential $EP` → `sqlx migrate run` against the **direct host**. The pooler would break advisory locks.
  2. Give the CI service principal its own role.
  3. Handle ownership: use `ALTER DEFAULT PRIVILEGES` or a shared GROUP role, so the app's service principal can use the tables CI created. Alternatively, run embedded `sqlx::migrate!()` at app start as the app's service principal (simplest).
  - If the workspace uses Private Link, Lakebase needs inbound Private Link, which means a self-hosted runner.
- **Analytics** [lb-sync], [lb-cdf]:
  - **Delta → Postgres synced tables are GA.** Use them for CRM and lead lists.
  - **Postgres → Delta CDF is Public Preview.** It writes `lb_<table>_history` tables (~15 s), needs `REPLICA IDENTITY FULL`, and is the natural route for call analytics in Databricks SQL and Genie.

## D. Rust and local-dev parity

### D1. Databricks from Rust
- **No official Rust SDK.** Official SDKs are Python, JS, Go and Java [sdks].
  - `community-databricks-sdk` 0.1.x is a month old and untested against a live workspace. Use it as a reference only [community-sdk].
- **Hand-roll a `TokenProvider`** (est. ~200 lines of code, reqwest):
  - Variants:
    - `Static` (PAT or local)
    - `M2m`: POST `/oidc/v1/token` with Basic `client_id:secret`, `grant_type=client_credentials&scope=all-apis`, `expires_in` 3600 [oauth-m2m].
    - `ScopedM2m`: adds `authorization_details` for route-optimized endpoints [route-opt-query].
    - `Federated`: RFC 8693 token exchange [wif].
  - Cache the token in an `ArcSwap` and refresh it ~5 min early, single-flight. On a 401, refresh once and retry.
  - Honour the `DATABRICKS_*` env vars and `DATABRICKS_AUTH_TYPE` [unified-auth].
- **LLM client:** `async-openai` with a custom `Config` that reads the bearer token from the `ArcSwap` on every call [async-openai].
- **Claude:** a small Anthropic Messages adapter.

### D2. Local equivalents and the wire contracts
| Capability | Databricks (prod) | Local (Mac) | Wire contract |
|---|---|---|---|
| LLM | FMAPI / Unity Gateway | **llama-server** with gpt-oss-20b GGUF (same weights as `databricks-gpt-oss-20b`). Ollama lacks `tool_choice` | OpenAI Chat Completions + SSE + tools [llama-server], [llama-fc], [ollama-openai] |
| STT (English) | Parakeet pyfunc, route-optimized | **mlx-audio** server (Parakeet/Whisper on MLX) behind a ~60-line shim | MLflow `{"inputs":[{audio_b64, sample_rate, language}]}` → `{"predictions":[{text}]}` |
| STT (multilingual) | Custom LLM Whisper-turbo | mlx-audio or WhisperKit `/v1/audio/transcriptions`, no shim | OpenAI transcriptions (multipart) [mlx-audio], [whisperkit] |
| TTS | Kokoro pyfunc | mlx-audio Kokoro, or Kokoro-FastAPI on MPS, behind the shim | MLflow `{"inputs":[{input, voice, response_format:"pcm", sample_rate}]}` → base64 PCM [kokoro-fastapi] |
| DB | Lakebase PG17 | `pgvector/pgvector:0.8.0-pg17` in Docker | Postgres; OAuth vs password is the only difference |

- **Adapter shape:** each adapter differs only in base URL, endpoint name and `TokenProvider`.
- **Why mirror the Databricks invocations envelope locally:** the local path then exercises the production body, URL and auth.
- whisper.cpp's `/inference` and speaches (which needs CUDA for real-time use) are weaker fits on a Mac [whisper.cpp], [speaches].

### D3. macOS setup
- **Yes: run the models natively on the host, and only Postgres in Docker.**
  - Docker on Mac has no GPU. Docker Model Runner runs llama.cpp natively with Metal, but **covers the LLM only, with no audio** [dmr].
  - Run the Rust server natively too: no `host.docker.internal` hops, faster rebuilds.
- **One command, `just dev`:**
  1. `docker compose up -d --wait db`
  2. `process-compose up` [process-compose], which starts `llama-server`, `mlx_audio.server`, the shim, `edge-relay` and `vak-server`.
  3. `just models` prefetches the weights.
- **Config:** `.env.local` and `.env.databricks` differ only in URLs and auth.
- **CI (Linux, no GPU):** a compose profile with CPU models (small GGUF, whisper.cpp base, Kokoro ONNX), or deterministic fake adapters.

## E. Synthesis

### E1. Recommended architecture
See the diagrams above. The core decisions:
1. **One Rust app on Apps** (Large, 2 instances) owns the routers, services and the voice pipeline. VAD and turn detection run in-process.
2. **Edge relay** for all public ingress.
3. **Two warm A10G custom endpoints** (STT Parakeet, TTS Kokoro), route-optimized.
4. **FMAPI LLM** through Unity Gateway with fallback.
5. **Lakebase** with direct connections and token refresh, plus CDF to Delta for analytics.
6. **Per environment:** a separate workspace and bundle target. Non-prod uses scale-to-zero endpoints and Lakebase branches.

### E2. Latency budget (phone; browser saves ~150 ms of transport)
| Stage | P50 | P95 | Notes |
|---|---|---|---|
| PSTN → carrier → relay → App | 100 | 200 | Relay adds ~1–5 ms in the same region (est.) |
| End-of-turn (Silero + Smart Turn, in-app) | 300 | 600 | No STT-native end-of-turn on Databricks |
| STT final (Parakeet on A10G, 3 s utterance, plus hop) | 70 | 200 | Whisper-turbo/vLLM ≈ 150–350 ms (est.) |
| LLM TTFT + first clause (FMAPI pay-per-token, reasoning off) | 450 | 1,000+ | No published TTFT; shared capacity |
| TTS first audio (Kokoro, first clause, plus hop) | 120 | 250 | Unary request, base64 PCM |
| Orchestration, resampling, μ-law | 20 | 50 | |
| App → relay → carrier → ear | 100 | 200 | |
| **Mouth-to-ear** | **~1,160 ms** | **~2.5 s** | Best case ~0.9 s |

**Levers toward 1 s:**
- Speculative STT and LLM start on VAD silence (pseudo-streaming).
- First TTS chunk ≤ 8 words, with pre-synthesized fillers.
- The fastest model that passes evals (GPT-OSS-20B or Haiku 5.5, reasoning off).
- Prompt caching.
- Provisioned throughput if pay-per-token tails hurt.

### E3. Cost per call-minute (est.)
**Fixed costs per month:**
- **HA setup ≈ $5.3k:**
  - Apps Large ×2: $1,080
  - STT A10G ×2: $2,044
  - TTS A10G ×2: $2,044
  - Lakebase 1–2 CU always-on: $50–100
  - Relay: ~$50
- **Lean setup ≈ $2.2–2.7k:** one of each, with TTS possibly on a T4.

**Variable costs ≈ $0.014/min:**
- Telnyx ~$0.0105 (doc 01)
- LLM ~$0.002 (Haiku 5.5: ~18k input + 300 output tokens per call-minute)
- Answering-machine detection (AMD): ~$0.001

| Monthly minutes | HA | Lean | Vendor-API stack (doc 01) |
|---|---|---|---|
| 20k | ~$0.28/min | ~$0.14 | $0.047 |
| 100k | ~$0.067 | ~$0.039 | $0.047 |
| 300k | ~$0.032 (may need more GPUs) | ~$0.023 | $0.047 |

**Break-even** against vendor APIs: about 75k (lean) to 160k (HA) minutes per month. Below that, warm GPUs are a fixed tax.

## Risks and spikes

| # | Risk | Mitigation |
|---|---|---|
| R1 | No public ingress, so the "all on Databricks" constraint can't be met | Edge relay (approval needed). Keep it stateless and tiny; same region |
| R2 | Long-lived WebSockets through the Apps proxy are unproven (~120 s HTTP timeout reported) | Spike S2. Fallback: outbound-dial tunnel (App → relay) or LiveKit |
| R3 | No streaming STT/TTS, so sub-1 s is at risk | Pseudo-streaming STT, sentence-chunked TTS, fillers. If S3/S4 fail, propose an exception for vendor streaming STT/TTS behind the same traits (doc 01 stack) |
| R4 | Warm-GPU cost floor; GPU capacity not guaranteed; maintenance reloads | N+1 replicas, no scale-to-zero in prod; T4 for Kokoro if quality holds |
| R5 | FMAPI shared capacity: no TTFT SLA, Haiku 5.5 limited to 20K output tokens/min, 503s | Unity Gateway fallback; small `max_tokens`; provisioned throughput for GPT-OSS/Llama |
| R6 | Apps: at most 5 fixed instances; `SIGTERM` +15 s drops live calls | Relay reconnect + `X-Routing-Key`; deploy windows; ask Databricks to raise limits |
| R7 | Preview dependencies: custom LLM serving (PP, US-only), CDF (PP), protobuf input (PP), guardrails (Beta) | Prefer GA paths (pyfunc + route optimization); pick us-east-1 or us-west-2 |
| R8 | CPU architecture and glibc undocumented; 10 MB file limit | Static musl wheel via Volume (S1) |
| R9 | 8 kHz telephony WER unknown for the models we'd deploy | Evaluate on real μ-law recordings in S3 |

**Spikes before committing** (in order, about 2 weeks total):
- **S1 (1 d). Rust on Apps:**
  - musl wheel from a Volume, then `uname -m`, h2c, `SIGTERM`, startup time.
  - Bind Lakebase and a serving endpoint as resources.
- **S2 (2 d). WebSocket ingress:**
  - Browser with SSO cookie, plus relay → `/api/media` with a service-principal bearer token.
  - 30-minute sessions with 20 ms frames, across 2 instances.
  - Measure added latency, idle and maximum-lifetime limits, and behaviour during a redeploy.
- **S3 (2–3 d). STT on Model Serving:**
  - Parakeet pyfunc (route-optimized) vs Whisper-turbo on custom LLM serving, both on A10G.
  - p50/p99 latency from the app for 1–4 s utterances, concurrency per GPU, WER on 8 kHz μ-law.
- **S4 (2 d). TTS on Model Serving:**
  - Kokoro pyfunc on A10G vs T4.
  - First-clause latency, base64 vs protobuf, concurrency per GPU.
- **S5 (1 d + 24 h soak). LLM:**
  - TTFT distribution for Haiku 5.5 vs GPT-OSS-20B/120B vs Llama 3.1 8B, measured from the app.
  - 429/503 rates; tool-call parity with llama-server.
- **S6 (2 d). End-to-end:**
  - Telnyx → relay → App → Model Serving.
  - Mouth-to-ear on 50 scripted calls, compared with the doc 01 vendor stack.
- **S7 (0.5 d). Lakebase:**
  - sqlx token refresh under load; branch-per-PR from GitHub OIDC (compute start time, role inheritance).


## Sources

Base URLs: `DA` = https://docs.databricks.com/aws/en/dev-tools/databricks-apps/ · `MS` = https://docs.databricks.com/aws/en/machine-learning/ · `LB` = https://docs.databricks.com/aws/en/oltp/projects/ · `GW` = https://docs.databricks.com/aws/en/ai-gateway/

- **Apps:** [system-env] DA/system-env · [app-runtime] DA/app-runtime · [deploy] DA/deploy · [app-development] DA/app-development · [dependencies] DA/dependencies · [best-practices] DA/best-practices · [compute-size] DA/compute-size · [horizontal-scaling] DA/horizontal-scaling · [key-concepts] DA/key-concepts · [permissions] DA/permissions · [connect-local] DA/connect-local · [embed] DA/embed · [auth] DA/auth · [http-headers] DA/http-headers · [networking] DA/networking · [resources] DA/resources · [env-vars] DA/environment-variables · [uc-volumes] DA/uc-volumes · [apps-lakebase] DA/lakebase · [observability] DA/observability · [cicd] DA/cicd-github-actions
- **Platform:** [bundle-app] https://docs.databricks.com/aws/en/dev-tools/bundles/resources · [limits] https://docs.databricks.com/aws/en/resources/limits · [apps-pricing] https://www.databricks.com/product/pricing/databricks-apps
- **Release notes:** https://docs.databricks.com/aws/en/release-notes/product/2026/{march,april,july,september,october}, cited as [rn-mar] [rn-apr] [rn-jul] [rn-sep] [rn-oct]
- **Community:** [community-504] https://community.databricks.com/t5/data-engineering/databrciks-app-504-upstream-request-timeout/td-p/146746 · [community-120s] https://community.databricks.com/t5/generative-ai/how-to-increase-http-request-timeout-for-databricks-app-beyond/td-p/140801 · [labs-ws] https://github.com/databrickslabs/coding-agents-databricks-apps/blob/main/docs/prd/websocket-terminal-io.md
- **Voice blueprint:** [db-voice-blog] https://www.databricks.com/blog/voice-agents-are-here-context-turns-them-real-customer-value · [diva] https://github.com/datasciencemonkey/diva (personal repo linked from that blog) · [diva-deploy] https://github.com/datasciencemonkey/diva/blob/main/skills/building-voice-agents-on-databricks/references/deployment.md
- **Third party:** [axum] https://github.com/tokio-rs/axum/blob/v0.8.x/axum/CHANGELOG.md · [twilio-ws] https://www.twilio.com/docs/voice/media-streams/websocket-messages · [telnyx-stream] https://developers.telnyx.com/api-reference/call-commands/streaming-start
- **FMAPI:** [fm-models] MS/foundation-model-apis/supported-models · [fmapi] MS/foundation-model-apis/ · [priority] MS/foundation-model-apis/priority-mode · [fm-limits] MS/foundation-model-apis/limits · [fm-api-ref] MS/foundation-model-apis/api-reference
- **Model Serving:** [anthropic-messages] MS/model-serving/query-anthropic-messages · [function-calling] MS/model-serving/function-calling · [structured-outputs] MS/model-serving/structured-outputs · [reason-models] MS/model-serving/query-reason-models · [audio-models] MS/model-serving/query-audio-video-models · [custom-llm] MS/model-serving/serve-custom-llms · [custom-models] MS/model-serving/custom-models · [endpoints] MS/model-serving/create-manage-serving-endpoints · [score-custom] MS/model-serving/score-custom-model-endpoints · [serving-limits] MS/model-serving/model-serving-limits · [serving-timeouts] MS/model-serving/model-serving-timeouts · [route-opt] MS/model-serving/route-optimization · [route-opt-query] MS/model-serving/query-route-optimization
- **Unity Gateway:** [model-regions] https://docs.databricks.com/aws/en/unity-gateway/model-region-availability · [ug] GW · [ug-query] GW/query-model-services · [ug-services] GW/model-services · [ug-traffic] GW/configure-traffic-splitting · [ug-inference-tables] GW/inference-tables · [service-policies] https://docs.databricks.com/aws/en/data-governance/unity-catalog/service-policies/
- **Agents, notebooks, functions:** [agent-stream] https://docs.databricks.com/aws/en/agents/custom-agents/model-serving/author-agent-model-serving · [whisper-nb] https://docs.databricks.com/aws/en/notebooks/source/machine-learning/custom-llm-serving/whisper-large-v3-turbo.html · [ai-transcribe] https://docs.databricks.com/aws/en/sql/language-manual/functions/ai_transcribe
- **Pricing:** [ms-pricing] https://www.databricks.com/product/pricing/model-serving · [fm-pricing] https://www.databricks.com/product/pricing/foundation-model-serving · [pfm-pricing] https://www.databricks.com/product/pricing/proprietary-foundation-model-serving
- **Open models:** [asr-board] https://huggingface.co/spaces/hf-audio/open_asr_leaderboard · [parakeet] https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2 · [kokoro] https://huggingface.co/hexgrad/Kokoro-82M · [kokoro-fastapi] https://github.com/remsky/Kokoro-FastAPI · [orpheus] https://github.com/canopyai/Orpheus-TTS · [chatterbox] https://github.com/resemble-ai/chatterbox · [kyutai] https://github.com/kyutai-labs/delayed-streams-modeling · [piper] https://github.com/OHF-Voice/piper1-gpl
- **Lakebase:** [lb-upgrade] https://docs.databricks.com/aws/en/oltp/upgrade-to-autoscaling · [lb-projects] LB/manage-projects · [lb-ext] LB/extensions · [lb-branches] LB/branches · [lb-manage-branches] LB/manage-branches · [lb-s2z] LB/scale-to-zero · [lb-computes] LB/manage-computes · [lb-auth] LB/authentication · [lb-roles] LB/postgres-roles · [lb-manual] LB/external-apps-manual-api · [lb-conn] LB/connection-strings · [lb-pool] LB/connection-pooling · [lb-apps-tutorial] LB/tutorial-databricks-apps-autoscaling · [lb-sync] LB/sync-tables · [lb-cdf] LB/lakebase-cdf · [lb-rn] https://docs.databricks.com/aws/en/release-notes/lakebase/ · [lb-pricing] https://www.databricks.com/product/pricing/lakebase · [gh-oidc] https://docs.databricks.com/aws/en/dev-tools/auth/provider-github · [sqlx-3117] https://github.com/transact-rs/sqlx/issues/3117 · [sqlx-pool] https://github.com/launchbadge/sqlx/blob/v0.8.6/sqlx-core/src/pool/mod.rs
- **Rust auth:** [sdks] https://docs.databricks.com/aws/en/dev-tools/sdks · [oauth-m2m] https://docs.databricks.com/aws/en/dev-tools/auth/oauth-m2m · [unified-auth] https://docs.databricks.com/aws/en/dev-tools/auth/unified-auth · [wif] https://docs.databricks.com/aws/en/dev-tools/auth/oauth-federation-exchange · [community-sdk] https://github.com/OffeneDatenmodellierung/databricks-rs · [async-openai] https://docs.rs/async-openai/latest/async_openai/config/trait.Config.html
- **Local models:** [llama-server] https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md · [llama-fc] https://github.com/ggml-org/llama.cpp/blob/master/docs/function-calling.md · [ollama-openai] https://docs.ollama.com/api/openai-compatibility · [mlx-audio] https://github.com/Blaizzy/mlx-audio · [whisperkit] https://github.com/argmaxinc/WhisperKit · [whisper.cpp] https://github.com/ggml-org/whisper.cpp · [speaches] https://github.com/speaches-ai/speaches
- **Tooling:** [dmr] https://docs.docker.com/ai/model-runner/ · [process-compose] https://github.com/F1bonacc1/process-compose
