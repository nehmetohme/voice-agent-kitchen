# 01 — Voice pipeline architecture and model options

> Researched 2026-10-30. Revised for the user constraint: **in production, STT, TTS and LLM run on Databricks Model Serving; local development uses local models.** Telephony stays external.
> Hosted APIs (Deepgram, Cartesia, ElevenLabs, …) appear here only as latency and quality **benchmarks**, and as an optional fallback if policy allows.
> Databricks mechanics (serving limits, streaming support, auth, pricing) are covered in [`07-databricks-platform.md`](07-databricks-platform.md).
> "AA" means Artificial Analysis. "Board" means the Hugging Face Open ASR Leaderboard. Numbers marked *vendor* are self-reported. Re-verify before committing.

## TL;DR

- **Architecture.** Use a **cascaded pipeline** (STT → LLM → TTS) orchestrated by the Rust backend. STT, TTS and the LLM are **open-weight models**:
  - production: GPU endpoints on Databricks;
  - development: native local servers on Apple Silicon.

  Both environments implement **the same versioned HTTP contracts**, so we write one adapter per role and switch base URLs per environment.
- **MVP models.**
  - **STT:** Parakeet TDT 0.6B v2 per utterance. Move to Nemotron Speech Streaming EN 0.6B if streaming serving works. Use Qwen3-ASR-1.7B for multilingual.
  - **TTS:** Kokoro-82M, with an identical ONNX file in dev and prod. Before GA, run a quality bake-off between Qwen3-TTS-1.7B, Ming-3.0-tts and Chatterbox Flash.
  - **LLM:** Qwen3.6-35B-A3B with thinking off.
  - **Turn detection:** Silero VAD plus Smart Turn v3, run inside the Rust process.
- **Latency.**
  - P50 mouth-to-ear target: ≤ **0.9 s** if the STT and TTS endpoints stream, ≈ **1.1–1.2 s** if they are request/response. P95 ≤ 1.6 s. The industry median is 1.4–1.7 s.
  - **The LLM endpoint must stream tokens.** STT and TTS can be request/response if we segment with VAD and chunk TTS by clause (§5).
- **Biggest compromise: TTS quality.** The best permissively licensed open TTS is Ming-3.0 at AA #24. Hosted Cartesia is #4 and ElevenLabs is #1, and Kokoro sits at #62. Everything above #24 among open-weight models is non-commercial or revenue-capped.
- **Cost.**
  - Telephony is unchanged: Telnyx about $0.0105/min, Twilio about $0.0184/min.
  - AI cost becomes GPU time: **$/call-min = GPU $/h ÷ (60 × concurrent calls per GPU × utilization)**.
  - Voice cannot tolerate cold starts, so there is an **always-on GPU floor**. At low volume, self-hosting costs more than hosted APIs (§11).
- **Transport and local dev.**
  - Browser test: WebSocket audio first; WebRTC (`str0m`) when the web widget ships. No self-hosted SIP for the MVP.
  - **Docker containers on macOS cannot use the Apple GPU.** Run local model servers natively on the host. Docker Model Runner covers only the LLM, via llama.cpp with Metal.

## Recommendation

| Area | Recommendation | Confidence |
|---|---|---|
| Architecture | Cascaded, behind one `VoiceEngine` trait. Speech-to-speech is out of production scope: hosted S2S breaks the constraint, and open S2S models lack tool calling | **High** |
| Serving contract | Our own versioned contracts: STT `transcribe` (utterance) plus optional `stream`; TTS `synthesize` (chunk) plus optional `stream`; LLM is OpenAI-compatible streaming chat. The same contract on Databricks and locally | **High** |
| Turn detection | Silero VAD + Smart Turn v3 (BSD-2), run in the Rust process through `ort`. Add Parakeet Realtime EOU 120M if STT streams | **High** |
| STT, English | Parakeet TDT 0.6B v2 per utterance, then Nemotron Speech Streaming EN 0.6B once streaming serving is proven | Medium-high |
| STT, multilingual | Qwen3-ASR-1.7B per utterance. Voxtral Mini 4B Realtime or Nemotron 3.5 ASR Streaming if streaming | Medium |
| TTS | Kokoro-82M for the MVP (fixed voices, exact parity). Production bake-off: Qwen3-TTS-1.7B, Ming-3.0-tts, Chatterbox Flash (English with cloning), CosyVoice 3 | **Medium** |
| LLM | Qwen3.6-35B-A3B with `enable_thinking:false`; alternative Gemma 4 26B-A4B. Avoid gpt-oss for voice (thinking can't be turned off). Whether Databricks-hosted proprietary models are allowed is answered in report 07 | Medium |
| Telephony | Telnyx primary, Twilio secondary. Both are external to Databricks | Medium-high |
| Browser transport | WebSocket for M1, then `str0m` WebRTC for the web widget | Medium-high |
| Self-hosted SIP | Not for the MVP | **High** |

---

## 1. Pipeline architecture: cascaded vs speech-to-speech

| Dimension | Cascaded | Speech-to-speech |
|---|---|---|
| Fits the Databricks constraint | **Yes.** Every stage is an open-weight model | Hosted APIs (OpenAI Realtime, Gemini Live, grok): **no**. Open-weight S2S (e.g. Kyutai Moshi) has **no tool calling** |
| Following a plan or script, guardrails | High. A text checkpoint sits between stages; the flow state machine swaps prompts and tools per node; output can be checked before it is spoken | Lower. All models score ≤ 75% on AA's τ-Voice; developers report instruction leakage |
| Tool calls | Mature text tool calling | Weaker; failures reported over SIP |
| Latency | 0.9–1.2 s P50 achievable | AA time to first audio: 0.57–1.3 s. Not dramatically faster |
| Observability | Every stage is logged and timed, and the transcript is exactly what the LLM saw | Partial. OpenAI's input transcript comes from a separate ASR model |

**One trait, at the engine level.** Transport, turn detection, the plan/flow state machine, tools, recording and metrics live *outside* the engine. That keeps a future S2S engine possible. Providers expose **both** request/response and streaming modes. Generic adapters turn request/response providers into streams (§5), so the pipeline never cares which mode a provider uses.

```rust
pub trait VoiceEngine: Send { fn start(&self, cfg: EngineConfig) -> BoxFuture<'static, Result<EngineHandle>>; }
pub trait SttProvider: Send + Sync {
    fn caps(&self) -> SttCaps;                                         // streaming? eou? word timestamps? languages
    async fn transcribe(&self, utt: Utterance) -> Result<Transcript>;  // request/response (Databricks or local)
    async fn open_stream(&self, cfg: SttConfig) -> Result<SttStream>;  // optional; default impl = BatchSttAsStream
}
pub trait TtsProvider: Send + Sync {
    async fn synthesize(&self, chunk: TextChunk, voice: &VoiceRef, fmt: AudioFormat) -> Result<SynthAudio>;
    async fn open_stream(&self, voice: &VoiceRef, fmt: AudioFormat) -> Result<TtsStream>; // default = ChunkedTtsAsStream
}
pub trait LlmProvider: Send + Sync { async fn stream(&self, ctx: &Context, tools: &[Tool]) -> Result<LlmStream>; }
```

## 2. Latency target, budget and turn-taking

**Reference points.**
- Twilio's cascaded launch target: 1,115 ms mouth-to-ear (limit 1,400 ms).
- Hamming, from 10M+ production minutes: P50 1.4–1.7 s.
- Callers notice delays above about 800 ms.

Our latency contract is measured mouth-to-ear on simulated calls in CI.

| Stage (per turn) | Streaming endpoints, P50 | Request/response endpoints, P50 | Evidence and levers |
|---|---|---|---|
| Inbound transport (PSTN → carrier → our WebSocket) | 100 ms | 100 ms | Twilio: ~95 ms for edge, buffer and decode |
| End-of-turn decision | 250 ms | 250 ms | About 200 ms of VAD silence, then Smart Turn v3 (10–65 ms on CPU). Parakeet EOU: p50 160 ms, p90 280 ms |
| Final transcript after end of turn | 30 ms | 100–200 ms | Streaming: Nemotron's 24 ms median time-to-final. Request/response: round trip plus inference on the *tail* segment only (§5) |
| LLM TTFT to first speakable clause | 300 ms | 300 ms | A 3B-active MoE prefills 3k tokens in about 0.11 s on an H100 (vLLM). Prefix caching cuts this further. Add serving overhead (report 07). **The LLM must stream** |
| TTS first audio | 100 ms | 150–300 ms | Qwen3-TTS 97 ms first packet; Chatterbox Flash ~110 ms; CosyVoice2 190 ms (L20); Kokoro ~300 ms (FastAPI, GPU). Request/response must synthesize the whole first clause |
| Orchestration and hops | 20 ms | 50 ms | One region. No hidden transcoding. Request the final wire format from the endpoint |
| Outbound transport | 100 ms | 100 ms | 20 ms frames; minimal buffering |
| **Mouth-to-ear** | **~900 ms** | **~1.05–1.3 s** | P95 ≤ 1.6 s. Tool calls are excluded and masked with cached filler audio |

**Turn detection.** Open STT models rarely emit end-of-turn signals, so the turn detector is ours.

| Approach | Model | License | Notes | Rust |
|---|---|---|---|---|
| Silence VAD | Silero VAD (8 and 16 kHz) | MIT | Always on, for speech gating and barge-in | `ort`, `silero-vad-rust` |
| Audio end-of-turn | **Smart Turn v3.x** | BSD-2 ✅ | 8 MB int8, 23 languages, 10–65 ms on CPU. Runs only during silence | ONNX via `ort` |
| Audio end-of-turn | LiveKit v1-mini; voxturn (0.8B, GPU) | Open weights; check terms | Multilingual false-cutoff rates (voxturn's report): Smart Turn v3.2 39.7%, v1-mini 28.4%, voxturn 19.9% | ONNX |
| Streaming STT with end-of-utterance | Parakeet Realtime EOU 120M; Kyutai stt-1b semantic VAD | NVIDIA OML / CC-BY-4.0 | English (Kyutai also French). Signal comes with the transcript | parakeet-rs / moshi |
| Hosted reference | Deepgram Flux; LiveKit v1 | n/a | Benchmarks at a 300 ms budget: 12.9% and 9.9% false cutoffs (LiveKit eot-bench) | n/a |

**Policy.**
1. Run VAD on every frame.
2. On ≥ 200 ms of silence, run Smart Turn v3 (or use the STT's end-of-utterance signal).
3. Commit after `min_delay` (0.3–0.5 s), or force a commit at `max_delay` (2.5–3 s).
4. Expose a per-agent **patience** setting, so surveys and booking can wait while callers dictate numbers.

**Barge-in.**
- VAD stays on while the agent speaks. Interrupt after ≥ 300–500 ms of speech, or ≥ 1–2 words that are not backchannels.
- On interrupt: stop playout, send `clear` to the carrier, cancel the LLM and TTS requests, bump the turn epoch, and **truncate the assistant message to what was actually played**.
- False-interruption resume: if no words arrive within about 1 s, carry on speaking.
- Use a per-language backchannel lexicon ("mm-hmm", "okay"). "Stop", "no" and "wait" interrupt at once.
- Agent-side backchannels are post-MVP.

## 3. STT: open-weight models for production and local

**Notes for reading the table.**
- WER is the Board's English short-form average (28 Oct 2026, H200, new normalizer). It is **not comparable** with older model-card averages or with AA's WER.
- RTFx means batch throughput, not streaming latency.

| Model | Params | License | Languages | WER EN (Board) | Streaming | GPU serving | Apple Silicon | Rust |
|---|---|---|---|---|---|---|---|---|
| **Nemotron Speech Streaming EN 0.6B** | 0.6B | NVIDIA Open Model License (custom; commercial OK) | EN | **6.60** | ✅ Cache-aware, 80 ms–1.12 s chunks; no end-of-utterance token | NeMo / Riva / Transformers. **560 streams per H100** at 320 ms; 24 ms median time-to-final | FluidAudio CoreML: 40.7× real time on M5 Pro; mlx-audio | parakeet-rs (`ort`); sherpa-onnx int8 |
| Nemotron 3.5 ASR Streaming 0.6B | 0.6B | OpenMDW-1.1 ✅ | 40 locales, auto language ID | 9.75 (multilingual 9.91) | ✅ | NeMo, NIM | FluidAudio, mlx-audio | parakeet-rs |
| Parakeet Realtime EOU 120M | 120M | NVIDIA OML | EN, no punctuation | card 9.30 | ✅ plus `<EOU>` (p50 160 ms) | NeMo | FluidAudio: 19× real time on M2 | parakeet-rs |
| Kyutai stt-1b-en_fr / stt-2.6b-en | 1B / 2.6B | CC-BY-4.0 ✅ | EN+FR / EN | 6.40 (2025) | ✅ 0.5 s / 2.5 s delay; semantic VAD (1B) | **Rust `moshi-server`**; 400 streams per H100 | moshi-mlx | `moshi` crates (candle) |
| Voxtral Mini 4B Realtime | 4B | Apache-2.0 ✅ | 13 | 8.44 (multilingual 9.36) | ✅ 80 ms–2.4 s delay | vLLM `/v1/realtime` WebSocket | mlx-audio 4-bit | Community only |
| **Qwen3-ASR 1.7B / 0.6B** | 1.7B / 0.6B | Apache-2.0 ✅ | 30 + 22 Chinese dialects | **5.40** / 6.24 (multilingual 6.56) | ~ chunked, vLLM only (WER worsens) | vLLM | mlx-audio | `qwen3-asr` (candle; Metal and CUDA) |
| **Parakeet TDT 0.6B v2 / v3** | 0.6B | CC-BY-4.0 ✅ | EN / 25 EU | 6.02 / 6.18 | ❌ Offline. RTFx ~6,000 | NeMo, Riva, TensorRT | parakeet-mlx, FluidAudio, mlx-audio | **parakeet-rs 0.4, sherpa-onnx**, transcribe-rs |
| Canary-Qwen-2.5B / Canary-1B-flash | 2.5B / 0.9B | CC-BY-4.0 ✅ | EN / 4 | 5.94 / 6.35 (2025) | ❌ | NeMo | mlx-audio (multilingual only) | transcribe-rs |
| Cohere Transcribe 03-2026 | 2B | Apache-2.0 (gated) | 14 | 5.77 | ❌ | vLLM | mlx-audio | community crates |
| Granite Speech 5.0 2B | 2B | Apache-2.0 (**the `-nc` variant is NC ❌**) | 6 | 5.68 | ❌ | vLLM | mlx-audio | n/a |
| Nemotron 3.5 Transcribe 30B-A4B | 30B (4B active) | OpenMDW ✅ | 40 | 5.33 (multilingual **4.69**, best open) | ❌ | vLLM, NIM | mlx-audio | n/a |
| Whisper large-v3-turbo | 809M | MIT ✅ | 99 | 7.64 | ❌ 30 s windows. Pseudo-streaming has ~3.3 s latency | faster-whisper, vLLM | **whisper.cpp** (Metal + Core ML); mlx-whisper | `whisper-rs` 0.16 |
| Moonshine Streaming small/medium | 123M / 245M | MIT ✅ | EN | card 7.84 / 6.65 | ✅ 80 ms lookahead | CPU / edge | Moonshine Voice | transcribe-rs |
| k2 streaming Zipformer | 20–97M | Mostly Apache | Mostly EN | n/a | ✅ | CPU | sherpa-onnx | **`sherpa-onnx` 2.0.1** (official, Oct 2026) |

**Phone audio.**
- Every model expects 16 kHz input, so we upsample 8 kHz μ-law in Rust (`rubato`).
- Only Parakeet v2 publishes a μ-law figure: WER went from 6.05 to 6.32 (+4% relative).
- Kyutai 1B was fine-tuned on Fisher telephone speech. Voxtral Realtime reports 11.65% on SwitchBoard.
- **Build our own 8 kHz evaluation set** from consented test calls.

**Picks.**
- **Request/response, English:** Parakeet TDT v2. It is fast, documented to survive μ-law audio, and has the best Mac and Rust tooling.
- **Streaming, English:** Nemotron Speech Streaming EN, after legal reviews the NVIDIA Open Model License. Pair it with Smart Turn v3 or Parakeet EOU.
- **Multilingual:** Qwen3-ASR-1.7B per utterance, or Voxtral Realtime / Nemotron 3.5 Streaming if streaming.

**Hosted reference (benchmark targets only).** AA latency and WER are on AA's own set, not comparable with the Board.

| Provider | $/min | AA latency | AA WER | Notes |
|---|---|---|---|---|
| Deepgram Flux | 0.0077 | 0.02 s | 7.4% | Built-in end of turn |
| AssemblyAI U3.6 Pro | 0.0075 | 0.19 s | 3.6% | |
| Soniox v5 | 0.002 | 0.054 s | 4.5% | |
| ElevenLabs Scribe v2 RT | 0.0065 | 0.141 s | 3.6% | |
| MAI-Transcribe-2 | n/a | n/a | **2.5%** | Best on AA |

## 4. TTS: open-weight models for production and local

License key: ✅ commercial use OK; ⚠ custom terms. AA ranks are out of 100 models.

| Model | Params | License | Languages | Cloning | AA rank (Elo) | Streaming / first audio | GPU stack | Apple Silicon | Rust |
|---|---|---|---|---|---|---|---|---|---|
| **Ming-3.0-tts** (Oct 2026) | 0.5B (or MoE 1.3B active) | MIT ✅ | 7 + dialects | Zero-shot (clip + transcript) | **#24 (1187)**, top permissive model | PCM streaming; first-audio time ⚠ unpublished | vLLM-Omni, SGLang-Omni | mlx-audio | n/a |
| **Qwen3-TTS** 0.6B / 1.7B | 0.6–1.7B | Apache-2.0 ✅ | 10 | 3 s | Open weights not ranked | ✅ Streaming text in and audio out; 97 ms first packet (GPU ⚠ unnamed) | vLLM-Omni (offline for now) | mlx-audio `--stream` | n/a |
| CosyVoice 3 (0.5B) | 0.5B | Apache-2.0 ✅ | 9 + 18 dialects | Zero-shot, cross-lingual | English WER 2.24 | ✅ Streaming in and out; CV2 190 ms on L20 | vLLM, TRT-LLM + Triton | No MLX port | n/a |
| **Chatterbox Flash** / Turbo / Multilingual | 0.35–0.5B | MIT ✅ (Perth watermark) | EN / EN / 23 | > 5 s | Chatterbox #80 | Flash streams blocks, 103–118 ms, RTF ~0.1. Turbo and Multilingual: whole clips | PyTorch, FlashInfer | mlx-audio (Flash RTF 0.78 on M4) | Official Turbo ONNX → `ort` |
| Kyutai TTS 1.6B | 1.8B | CC-BY-4.0 ✅ | EN, FR | No (preset voices) | n/a | ✅ **Streaming text input**; 150 ms at batch 1, 403 ms at batch 64 (H100) | **Rust `moshi-server`** | moshi-mlx | `moshi` (candle) |
| Pocket TTS (Kyutai) | 100M | MIT / CC-BY-4.0 ✅ | 6 | From a WAV | n/a | ~200 ms; 6× real time on M4 CPU | CPU-first | MLX port | sherpa-onnx; official Rust port |
| **Kokoro-82M** | 82M | Apache-2.0 ✅ | 8 | No | #62 (1062) | Sentence chunks; ~300 ms (FastAPI, GPU) | Kokoro-FastAPI / ONNX | mlx-audio, ONNX | **`sherpa-onnx`**, Kokoros |
| Kitten v0.8 | 15–80M | Apache-2.0 ✅ | EN | No | n/a | CPU ONNX | n/a | mlx-audio | sherpa-onnx |
| Magpie Multilingual | 364M | NVIDIA OML ⚠ (revocable) | 12 | No (5 voices) | #58 | ⚠ | NeMo, Riva | Community CoreML | n/a |
| Orpheus 3B | 3.8B | Code Apache; weights ⚠ Llama-derived | EN | Unreliable | n/a | ~200 ms *vendor* | vLLM, TRT-LLM | mlx-audio, GGUF | candle `orpheus` + `snac` |
| CSM-1B / Dia2 | 1–2B | Apache-2.0 ✅ | EN | Audio context | n/a | CSM: no official streaming | PyTorch | mlx-audio | candle `csm` |
| Marvis v0.2 / NeuTTS Air | 250M / 552M | Apache-2.0 ✅ (NeuTTS Nano ⚠ $5M cap) | EN (+fr/de) | ~10 s / 3–15 s | n/a | Streams; no published numbers | n/a / llama.cpp | mlx-audio / GGUF | n/a |

**Exclude, because the license is non-commercial or capped:**
- Fish Drama 3: AA #6, the top open model. Capped at $500k revenue and hosted services are banned.
- Breeze TTS 2 (#13), Voxtral TTS (#48), Fish S2, Higgs v3, Spark-TTS, F5-TTS: non-commercial.
- XTTS-v2: Coqui CPML.
- Higgs v2: needs a license above 100k annual active users.
- Kitten-TTS 2: $1M cap.
- IndexTTS2: custom license.
- VibeVoice-Realtime: "research only".
- **Piper:** the code is now **GPL-3.0** and each voice has its own license.

**Output rate.** Native rates are 22–48 kHz. Ask the endpoint to return the final wire format (8 kHz μ-law for phone), or resample in Rust.

**Picks.**
- **MVP: Kokoro-82M.** The same ONNX file runs in Rust via sherpa-onnx, on a Mac, in CI on CPU, and on the GPU. Fixed voices only.
- **Production quality and cloning:** a TTFA and MOS bake-off between Qwen3-TTS-1.7B, Ming-3.0-tts and CosyVoice 3, plus **Chatterbox Flash** for English cloning.
- **Rust-native streaming:** Kyutai TTS 1.6B, if English and French are enough.

**Hosted reference (benchmark targets).** TTFB is Vapi's independent median.

| Model | AA rank | TTFB | $/min |
|---|---|---|---|
| ElevenLabs v4 Turbo | **#1** | 158 ms | 0.040 |
| Cartesia Sonic 3.6 | #4 | 101 ms | 0.04–0.05 |
| Inworld TTS-2 | #8 | 288 ms | 0.025 |
| Murf Falcon 2.1 | #29 | 117 ms | 0.010 |

Deepgram Flux TTS is also worth noting: on barge-in it returns `text_spoken` and `text_remaining`. Several hosted products are dead or dying: OpenAI's TTS models shut down on 2027-01-06, Hume's APIs end on 2026-11-13, and PlayHT is gone.

## 5. Pipeline design for request/response STT and TTS endpoints

Use this design if Databricks custom-model endpoints can't hold streaming WebSockets (see report 07). The rest of the pipeline is unchanged because two generic adapters present request/response providers as streams: `BatchSttAsStream` and `ChunkedTtsAsStream`.

**STT: VAD-segmented utterances.**
1. Silero VAD runs in Rust on 20 ms frames.
2. During a turn, **cut sub-segments at short pauses (≥ 150–200 ms)**. Send each finished sub-segment to `transcribe` straight away, as *pre-transcription*.
3. When the turn ends, only the **tail segment** is still outstanding. That keeps the final-transcript cost to one round trip plus inference on about 1–2 s of audio.
4. On ≥ 200 ms of silence, run Smart Turn v3 locally **in parallel with** the tail request.
   - "Complete": commit as soon as the transcript returns.
   - "Incomplete": keep waiting, up to `max_delay`.
5. Optional, off the critical path: re-transcribe the whole turn for the stored transcript (better WER for logs and QA).
6. Don't use LocalAgreement-style pseudo-streaming with Whisper. It runs at about 3.3 s latency.
7. Payload: 1 s of 16 kHz PCM16 is 32 KB, plus 33% if base64-encoded in JSON. Use a binary body if the endpoint allows it.

**TTS: clause-chunked synthesis.**
1. Aggregate the streamed LLM tokens. Make the **first chunk short**: cut at the first punctuation after ≥ 4 words, or at 10–12 words. Later chunks end at sentence boundaries, up to ~25 words.
2. Request chunk 1 immediately. Request chunk *n+1* when chunk *n* starts playing. A look-ahead of 1–2 chunks limits wasted GPU work when the caller interrupts.
3. Request the wire format (8 kHz μ-law) from the endpoint. That cuts the payload about 6× compared with 24 kHz PCM16 and takes resampling off the hot path.
4. Hide seams between chunks: cut at clause boundaries, fix voice and seed, and crossfade 5–10 ms.
5. **Cache fixed lines** keyed by (voice, text, format): greeting, AI disclosure, recording notice, fillers ("One moment…") and voicemail drops. They play with no TTS latency and stay byte-identical for compliance.

**Barge-in with request/response endpoints.**
- Detection stays local (VAD plus minimum duration), so it is unaffected.
- On an interrupt: clear the playout queue, send the carrier `clear`, and cancel in-flight HTTP calls (drop the futures or fire a `CancellationToken`). The GPU may finish the work anyway; a small look-ahead keeps that waste down.
- **Context truncation is easier with chunks.** We know which chunks played in full. For the interrupted chunk, estimate the spoken words as `played_ms / chunk_ms` of its words, or use alignment data if the endpoint returns it.
- Telling a backchannel from "stop" in a short overlap (< 500 ms): decide by duration alone, or run a quick `transcribe` on the clip (+100–200 ms).

**Effect on the latency budget.**
- About +100–200 ms for the STT tail, +50–200 ms for the first TTS chunk and +30 ms for extra hops: **+200–400 ms** in total (§2 table).
- **A non-streaming LLM would add its whole generation time (+0.5–1.5 s), which is unacceptable.** The LLM endpoint must stream.

## 6. LLM: open-weight models (production) and local equivalents

| Model | Params (active) | License | Tool calling | Thinking off? | Notes |
|---|---|---|---|---|---|
| **Qwen3.6-35B-A3B** (Apr 2026) | 35B (3B) | Apache-2.0 | TAU3 67.2, MCP-Atlas 62.8 | ✅ `enable_thinking:false` | 262k context. vLLM ≥ 0.19 / SGLang ≥ 0.5.10 with the `qwen3_coder` parser. Speculative decoding (MTP) |
| Gemma 4 26B-A4B | 25B (3.8B) | Apache-2.0 | τ2 68.2 (31B dense: 76.9) | ✅ omit `<\|think\|>` | 256k context. H100 prefill about 26k tok/s, so ~0.11 s for 3k tokens |
| Nemotron 3 Nano / **PhoneLLM alpha-1** (Pipecat) | 30B (3.5B) | NVIDIA OML / BSD-2 | PhoneBench 72.1 *vendor* | ✅ trained thinking-off | Vendor P95 TTFT < 100 ms (B200). Alpha |
| GLM-4.7-Flash | 30B (3B) | MIT | τ2 79.5 *vendor* | ⚠ undocumented | vLLM `glm47` parser |
| Mistral Small 4 | 119B (6.5B) | Apache-2.0 | n/a | ✅ `reasoning_effort:"none"` | Larger |
| gpt-oss-20b / 120b | 21B (3.6B) / 117B (5.1B) | Apache-2.0 | τ retail at "low": 35.0 / 49.4 | ❌ "low" is the minimum | **Avoid for voice.** Its only advantage: identical MXFP4 weights on Mac and GPU |

Vendors mostly report tool scores *with thinking on*. **Run our own thinking-off tool-calling eval** on agent scripts before choosing.

**Hosted reference for TTFT and quality:**
- Claude Haiku 5.5: $0.10/$0.50 per 1M tokens; no independent thinking-off TTFT yet.
- GPT-6-luna: 0.89 s.
- Cerebras gpt-oss-120b: 0.37 s.

## 7. Local development on Apple Silicon

| Component | Local (native macOS process) | Production (Databricks GPU) | Parity |
|---|---|---|---|
| VAD and turn detection | In the Rust process (`ort`: Silero, Smart Turn ONNX) | Same binary | Identical |
| STT | `parakeet-rs` / `sherpa-onnx` in-process, or FluidAudio / parakeet-mlx server | NeMo / Riva / Transformers | Same weights; ONNX vs PyTorch numerics |
| TTS | Kokoro via `sherpa-onnx`; Qwen3-TTS via mlx-audio | ONNX or vLLM-Omni | Kokoro is the same ONNX file. Qwen3 uses MLX quantization |
| LLM | `llama-server`, `mlx_lm.server`, Ollama or LM Studio, all with OpenAI-compatible streaming and tools. 4-bit Qwen3.6-35B-A3B (~20 GB) | vLLM / SGLang, BF16 or FP8 | Prompts and tools identical. Expect quantization drift |

**Measured Mac LLM TTFT.**
- 3k-token prompt, cold: about 1.7–2.6 s on M4 Max-class machines (Qwen3.5-35B-A3B under Ollama; gpt-oss-20b under llama.cpp).
- On this M4 Max (battery, Low Power Mode): Qwen3-30B-A3B took 7.0 s cold, but **0.26 s on a prefix-cache hit**.
- So **warm the prefix cache at call start**, and never judge latency on a laptop. The latency contract runs against a prod-like GPU.
- `mistral.rs` 0.9.5 offers in-process Rust inference with Metal and tools, but no Nemotron-H. Prefer calling a local server over HTTP, which keeps one code path.

**Rules.**
- **Containers on macOS get no Metal GPU access.**
  - Run model servers natively, for example with `just models`, and point Compose services at `host.docker.internal`.
  - Docker Model Runner can serve GGUF LLMs on the host GPU through llama.cpp and Metal, and can be declared in Compose. vLLM isn't supported on macOS, and Model Runner covers no STT or TTS.
- **One contract, two base URLs.** Rust adapters are identical across environments; only the endpoint path and auth config differ.
- **CI:** run CPU-only ONNX models (Kokoro, Parakeet int8, Silero, Smart Turn) for real end-to-end simulated calls without GPUs. Add mock providers with injected delays for the latency contract.

## 8. Telephony (external to Databricks)

| Provider | Media WebSocket | Codec | Out / in $/min | Stream fee | Number $/mo | AMD | STIR/SHAKEN | Rust notes |
|---|---|---|---|---|---|---|---|---|
| **Telnyx** | `streaming_start`, bidirectional | PCMU/PCMA, G722, **L16 16k**, Opus | **0.007 / 0.0052** | 0.0035 | 1.00 | $0.002 (premium $0.0065), beep detection | **A for owned numbers** | OpenAPI spec |
| **Twilio** | `<Connect><Stream>`, `mark`/`clear` | **μ-law 8k only** | 0.014 / 0.0085 | 0.0044 | 1.15 | $0.0075/call, async | **C by default.** A needs Trust Hub + SHAKEN product (~4 days) | OpenAPI (beta) + Rust codegen guide |
| Vonage | NCCO WebSocket | L16 8/16/24k | 0.0085 / 0.0049 | 0.0049 | 1.06 | $0.0084 | Unverified | n/a |
| Plivo | `<Stream bidirectional>` | μ-law, L16 | 0.0115 / 0.0055 | n/a | 0.50 | $0 | A for Plivo DIDs | n/a |
| SignalWire | Twilio-compatible | PCMU, L16 16/24k | 0.008 / 0.0066 | 0.003 | 0.50 | $0.006 | C by default | `signalwire-sdk` (young) |
| Bandwidth | `<StartStream>` (early access) | PCMU, PCM | 0.010 / 0.0055 | 0.004 | n/a | $0.006 | A automatically | n/a |

**Rust fit.** Every carrier is REST + webhooks + WebSocket, which `reqwest`, `axum` and `tokio-tungstenite` handle. Hand-write the ~8 endpoints we need:
- number search and order
- call create, hang up and transfer
- stream start
- AMD

**Outbound hygiene:**
- Get A attestation.
- Register numbers at the Free Caller Registry.
- Monitor spam labels.
- Use AMD with beep detection for voicemail drop.

**Bridging the carrier to Databricks Apps.** Auth and the WebSocket path are covered in report 07. For example, Twilio streams can't carry a bearer token, while Telnyx has `stream_auth_token`.

**Self-hosted SIP: no for the MVP.**
- It saves only about $0.0055/min against Telnyx.
- We would own RTP, jitter, DTMF, SBC, AMD and failover.
- Later options: Asterisk 23 `chan_websocket`, LiveKit SIP, or `rsipstack` 0.7 on an Elastic SIP trunk.

## 9. Browser "talk to your agent" transport

| Option | Pros | Cons | Rust fit |
|---|---|---|---|
| **Plain WebSocket** (PCM16 16 kHz or Opus, 20 ms frames, AudioWorklet) | Trivial. **Same shape as carrier media streams.** Works through proxies; no TURN server | TCP head-of-line blocking. We build playout buffering. Browser echo cancellation of Web Audio playback is reliable only in Chromium ("Chrome-wide AEC") | `axum` WebSocket |
| **WebRTC in Rust**: `str0m` 0.18 (sans-I/O) or `webrtc-rs` 0.21 (1.0 RC, Sept 2026) | UDP, Opus, browser AEC/NS/AGC, jitter buffer. The standard for a public widget | ICE/DTLS + TURN; more code; pre-1.0 crates | Good (`str0m` suits a tokio task per session) |
| LiveKit (SFU) + `livekit` crate | Mature, includes SIP | Extra service. The Rust SDK wraps libwebrtc (C++) | OK |
| Daily / Pipecat Cloud | Managed | No Rust SDK | Poor |

**Recommendation.**
- **M1:** WebSocket behind a `MediaTransport` trait. Testers use Chrome with headphones.
- **Web widget:** `str0m` + coturn.
- No SFU is needed: these are 1:1 sessions.

## 10. Reference frameworks: what to borrow

| Source | Pattern | Rust translation |
|---|---|---|
| **Pipecat** (Python 1.x) | Typed frames through linear processors. `SystemFrame`s take a high-priority lane and are never dropped. Data and control frames are ordered and interruptible. Errors flow upstream | `enum Frame`; one tokio task per stage; a bounded data channel plus a control channel read first with `select!{ biased; }` |
| Pipecat | Drop queued interruptible frames on interrupt | **Turn epoch**: every frame carries a `TurnId`, stale epochs are dropped, and a `CancellationToken` aborts in-flight HTTP |
| **Pipecat Flows** | Nodes (a task prompt plus allowed functions). Edge functions move to another node. Declarative branch tables | The **Plan** model: a pure state machine in `domain`, run by `services` |
| **LiveKit Agents** | One `AgentSession` for pipeline or realtime engines. Turn options: mode, `min`/`max_delay`, dynamic endpointing, interruption `min_duration`/`min_words`, false-interruption resume, preemptive generation, user turn limits | Copy this option surface as a per-agent `TurnPolicy` |
| Twilio latency guide | Colocate stages. Avoid hidden transcoding. Don't co-host STT and TTS on one GPU (load collisions cause tail spikes) | Keep separate STT and TTS endpoints; convert audio formats only at the edges |

Also add per-turn OpenTelemetry spans covering VAD stop → end of turn → STT final → first LLM token → first clause → first TTS byte → first audio out. These feed the **latency contract test**: recorded audio, mock providers with injected delays, and a paused tokio clock.

## 11. MVP stack, alternatives and cost

| Layer | Production (Databricks) | Local (Mac) | Alternatives |
|---|---|---|---|
| Telephony | Telnyx (external) | Telnyx dev numbers, or browser only | Twilio |
| Browser | WebSocket | Same | `str0m` WebRTC |
| VAD and turn detection | Silero + Smart Turn v3 (Rust, CPU) | Same | Parakeet EOU, LiveKit v1-mini |
| STT | Parakeet TDT 0.6B v2 (utterance) | parakeet-rs / sherpa-onnx | Nemotron Streaming EN; Qwen3-ASR-1.7B; Kyutai STT |
| LLM | Qwen3.6-35B-A3B, thinking off (vLLM/SGLang) | 4-bit build on llama-server / MLX | Gemma 4 26B-A4B; PhoneLLM; models Databricks hosts (report 07) |
| TTS | Kokoro-82M (ONNX) | sherpa-onnx | Qwen3-TTS, Ming-3.0, CosyVoice 3, Chatterbox Flash, Kyutai TTS |
| Hosted fallback (only if allowed) | Off | Off | Deepgram Flux, Cartesia, ElevenLabs |

**Cost per call-minute.**
- **Telephony:**
  - Telnyx: $0.0105/min, plus AMD ≈ $0.001/min on a 2-minute average call.
  - Twilio: $0.0184/min, plus AMD ≈ $0.0038/min.
- **AI on GPU:** per endpoint type, `G / (60 · C · U)`, where:
  - *G* is the GPU $/h from Databricks pricing (report 07);
  - *C* is concurrent calls per GPU;
  - *U* is average utilization.
- **Capacity hints:**
  - Nemotron streaming: 560 streams per H100.
  - Kyutai STT: 400 streams per H100.
  - Kyutai TTS: batch 64 at 403 ms per H100.
  - LLM decode load per call is tiny (about 4 turns × 60 tokens per minute) and the prompt prefix is cached, so a 3B-active MoE should serve dozens of calls per GPU. This is an estimate; benchmark it.
- **Illustrative only** (G = $4/h is a placeholder, not Databricks pricing):
  - With C = 40 and U = 0.5: ≈ $0.0033/min per endpoint, so ≈ **$0.01/min** for STT + TTS + LLM, and ≈ **$0.022/min all-in with Telnyx**.
  - **Always-on floor:** 3 GPUs × $4 × 730 h ≈ **$8.8k/month** before the first call. Scale-to-zero cold starts are unacceptable for voice.
  - The always-on GPUs cost the same whatever the call volume, up to their capacity. Against hosted APIs at about $0.035/min of AI cost, self-hosting **breaks even around 250k call-minutes a month** ($8.8k ÷ $0.035). Below that, the constraint costs money; the payoff is governance and control.

## Risks and open questions

**Decisions the user must make:**
1. **Streaming vs request/response serving**, once report 07 confirms what Model Serving supports. This sets the latency target: 0.9 s vs about 1.2 s.
2. **Launch languages.** English: Parakeet/Nemotron with Kokoro or Chatterbox. Multilingual: Qwen3-ASR with Qwen3-TTS.
3. **TTS quality bar.** Kokoro (simple, exact parity, no cloning, AA #62) or a GPU model (Qwen3-TTS / Ming / Chatterbox Flash: better quality and cloning, more serving work).
4. **Is voice cloning in scope?** It brings consent capture and watermarking (Chatterbox has Perth).
5. **License policy.** Is the NVIDIA Open Model License acceptable for Nemotron streaming? CC-BY-4.0 requires attribution. We need a **model-license allowlist checked in CI**, like `cargo-deny`.
6. **May hosted APIs act as an emergency fallback**, or only as offline benchmarks?
7. **Are Databricks-hosted proprietary LLMs allowed**, or open weights only?
8. **Carrier:** Telnyx or Twilio.
9. **GPU budget:** the always-on floor and the GPU type per endpoint.

**Risks:**
- **Serving overhead and limits on Databricks** (latency, concurrency, payload size, cold starts) are unknown here. That is report 07's job. Benchmark a round trip in week 1.
- **Open TTS quality gap.** Run a MOS listening test against Cartesia and ElevenLabs before GA.
- **Dev/prod drift:** 4-bit MLX vs BF16 GPU, and ONNX/CoreML vs PyTorch numerics. Mitigate by running golden-transcript and golden-reply tests in both environments.
- **Unverified numbers:** Ming's first-audio time; the GPUs behind the Qwen3-TTS and Chatterbox Flash figures; open STT latency per utterance (estimated tens of ms for Parakeet, 100–300 ms for Whisper-turbo); open LLM tool scores with thinking off.
- **Only Parakeet publishes phone-audio WER.** Build our own 8 kHz evaluation set.
- **Model serving in production will likely be Python** (NeMo, vLLM). That sits outside the 100%-Rust backend boundary and must be recorded in an ADR.
- **The model landscape changes monthly.** Pin a model version per published agent version, give every adapter recorded-fixture contract tests, and keep the trait boundaries strict.

## Sources

- **Latency and architecture:** https://www.twilio.com/en-us/blog/developers/best-practices/guide-core-latency-ai-voice-agents · https://hamming.ai/resources/voice-ai-latency-whats-fast-whats-slow-how-to-fix-it · https://deepgram.com/blog/voice-agent-architecture-stt-llm-tts-pipeline-design · https://github.com/ufal/whisper_streaming
- **Turn detection:** https://github.com/pipecat-ai/smart-turn · https://docs.pipecat.ai/api-reference/server/utilities/turn-detection/smart-turn-overview · https://docs.livekit.io/agents/logic/turns/turn-detector/ · https://livekit.com/blog/solving-end-of-turn-detection · https://livekit.com/blog/turn-detection-and-interruption-handling · https://docs.livekit.io/agents/logic/turns/tuning/ · https://huggingface.co/AustinXiao/voxturn · https://github.com/sheldonix/silero-vad-rust
- **Open STT:** https://huggingface.co/spaces/hf-audio/open_asr_leaderboard · https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b · https://huggingface.co/blog/nvidia/nemotron-speech-asr-scaling-voice-agents · https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b · https://huggingface.co/nvidia/parakeet_realtime_eou_120m-v1 · https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2 · https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3 · https://huggingface.co/nvidia/canary-qwen-2.5b · https://huggingface.co/nvidia/Nemotron-3.5-Transcribe-30B-A4B · https://huggingface.co/kyutai/stt-1b-en_fr · https://github.com/kyutai-labs/delayed-streams-modeling · https://huggingface.co/mistralai/Voxtral-Mini-4B-Realtime-2602 · https://huggingface.co/Qwen/Qwen3-ASR-1.7B · https://huggingface.co/CohereLabs/cohere-transcribe-03-2026 · https://huggingface.co/ibm-granite/granite-speech-5.0-2b · https://github.com/moonshine-ai/moonshine · https://github.com/SYSTRAN/faster-whisper
- **Open TTS:** https://artificialanalysis.ai/text-to-speech/leaderboard · https://huggingface.co/inclusionAI/Ming-3.0-tts-0.5B · https://github.com/QwenLM/Qwen3-TTS · https://huggingface.co/FunAudioLLM/Fun-CosyVoice3-0.5B-2512 · https://github.com/resemble-ai/chatterbox · https://huggingface.co/ResembleAI/chatterbox-flash · https://huggingface.co/kyutai/tts-1.6b-en_fr · https://github.com/kyutai-labs/pocket-tts · https://huggingface.co/hexgrad/Kokoro-82M · https://github.com/remsky/Kokoro-FastAPI · https://github.com/KittenML/KittenTTS · https://huggingface.co/nvidia/magpie_tts_multilingual_357m · https://github.com/canopyai/Orpheus-TTS · https://huggingface.co/sesame/csm-1b · https://huggingface.co/fishaudio/drama-3 · https://huggingface.co/mistralai/Voxtral-4B-TTS-2603 · https://github.com/OHF-Voice/piper1-gpl · https://huggingface.co/coqui/XTTS-v2/blob/main/LICENSE.txt
- **Open and local LLMs:** https://huggingface.co/Qwen/Qwen3.6-35B-A3B · https://huggingface.co/google/gemma-4-26b-a4b-it · https://huggingface.co/pipecat-ai/phonellm-alpha-1 · https://huggingface.co/zai-org/GLM-4.7-Flash · https://huggingface.co/mistralai/Mistral-Small-4-119B-2603 · https://arxiv.org/html/2508.10925 · https://github.com/ggml-org/llama.cpp/discussions/15396 · https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md · https://ollama.com/blog/mlx · https://machinelearning.apple.com/research/exploring-llms-mlx-m5 · https://lmstudio.ai/docs/developer/openai-compat/tools · https://docs.mistralrs.dev/reference/supported-models/
- **Apple Silicon and Rust runtimes:** https://github.com/Blaizzy/mlx-audio · https://github.com/FluidInference/FluidAudio/blob/main/Documentation/Benchmarks.md · https://github.com/senstella/parakeet-mlx · https://github.com/ggml-org/whisper.cpp · https://github.com/altunenes/parakeet-rs · https://github.com/cjpais/transcribe-rs · https://crates.io/crates/sherpa-onnx · https://github.com/k2-fsa/sherpa-onnx/tree/master/rust-api-examples · https://github.com/huggingface/candle · https://docs.docker.com/ai/model-runner/inference-engines/
- **Hosted reference (STT/TTS/LLM/S2S):** https://deepgram.com/pricing · https://www.assemblyai.com/pricing · https://soniox.com/pricing · https://artificialanalysis.ai/speech-to-text/streaming · https://humannessindex.vapi.ai/ · https://cartesia.ai/pricing · https://elevenlabs.io/pricing/api · https://inworld.ai/pricing · https://murf.ai/api/docs/text-to-speech-models/falcon · https://developers.deepgram.com/docs/flux-tts/feature-overview · https://developers.openai.com/api/docs/deprecations · https://dev.hume.ai/docs/introduction · https://platform.claude.com/docs/en/about-claude/pricing · https://developers.openai.com/api/docs/pricing · https://artificialanalysis.ai/speech-to-speech · https://artificialanalysis.ai/leaderboards/models
- **Frameworks and WebRTC:** https://docs.pipecat.ai/pipecat/learn/pipeline · https://docs.pipecat.ai/api-reference/server/frames/system-frames.md · https://docs.pipecat.ai/pipecat/flows/functions · https://github.com/livekit/rust-sdks · https://github.com/algesten/str0m · https://github.com/webrtc-rs/webrtc · https://webrtc.rs/blog/ · https://devforum.zoom.us/t/issupportchromewideaec-sniffs-browser-name-version-breaking-echo-cancellation-on-microsoft-edge/145288
- **Telephony:** https://telnyx.com/pricing/call-control · https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming · https://developers.telnyx.com/docs/voice/programmable-voice/answering-machine-detection · https://developers.telnyx.com/docs/voice/stir-shaken/attestation-behavior · https://developers.telnyx.com/api-reference/phone-number-search/list-available-phone-numbers · https://www.twilio.com/en-us/voice/pricing/us · https://www.twilio.com/docs/voice/media-streams/websocket-messages · https://www.twilio.com/docs/voice/answering-machine-detection · https://www.twilio.com/docs/voice/trusted-calling-with-shakenstir/shakenstir-onboarding · https://www.twilio.com/docs/openapi/generating-a-rust-client-for-twilios-api · https://www.vonage.com/communications-apis/voice/pricing/ · https://plivo.com/docs/voice/xml/audio-streaming · https://signalwire.com/pricing/voice · https://dev.bandwidth.com/docs/voice/programmable-voice/bxml/startStream · https://www.freecallerregistry.com/fcr/ · https://docs.asterisk.org/Configuration/Channel-Drivers/WebSocket/ · https://docs.livekit.io/sip/ · https://github.com/restsend/rsipstack
