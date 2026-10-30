# 09: NVIDIA Nemotron 3.5 Bolt (E4B, 74B-A9B) for Voice Agent Kitchen

> Researched 2026-10-30. Bolt 74B-A9B shipped 2026-10-15 and Bolt E4B on 2026-10-23, so every source is under three weeks old.
> - *vendor* marks a number NVIDIA measured, including NVIDIA's measurements of Qwen and Gemma. **No benchmark below has been reproduced independently** unless marked *indep.*
> - "est." marks my own estimate.
> - Bracketed keys point to **Sources**. Docs 01, 07 and 08 are the sibling reports in this folder.

## TL;DR

- **Bolt E4B** is NVIDIA's RL post-train of Google's Gemma 4 E4B: 8B total / 4B effective parameters, text-only by its model card, 64K context. It needs about 4 GB of memory at Q4.
  - Thinking toggle (**on by default**), a `conversational_mode` for short spoken replies, and tool calling.
  - License: OpenMDW-1.1. Commercial and SaaS use are allowed. [e4b-card] [openmdw]
- **It is good at personas and conversation, and weak at multi-turn customer-service tool use.**
  - Strong on RoleBench, IHEval, brevity and IFBench.
  - **On Tau2-Bench with thinking off it scores 34.3, against 73.5 for Qwen3.5-4B** (55.1 vs 75.1 with thinking on). These are NVIDIA's own numbers. [e4b-tr]
  - Artificial Analysis has not scored either Bolt model yet [aa-74b].
- **The main-LLM recommendation does not change: keep Qwen3.6-35B-A3B (thinking off).**
  - E4B: too weak on Tau2.
  - Bolt 74B-A9B:
    - NVIDIA published reasoning-on evaluations only.
    - Its IFBench score (66.2) trails Gemma 4 26B-A4B (77.3).
    - It decodes slower than both Qwen and Gemma (84 vs 97 and 101 tok/s).
    - Its 2-bit kernels need SM12x Blackwell; on Databricks that means the RTX PRO 6000 only. [b74-blog] [b74-tr] [b12x]
- **Best fit:**
  - **MVP:** E4B as the **simulated caller** in tests and pre-publish checks, plus a light local/CI profile.
  - **Later:** first responder, edge/on-prem deployments, slot extraction, and a seat for Bolt 74B in the main-LLM bake-off.
  - **Routing and guardrails stay with Jev System One** (doc 08).
- **Databricks.** Neither Bolt model is in Foundation Model APIs or `system.ai` (zero "Nemotron" hits on 2026-10-30) [dbx-models].
  - Deploy E4B through **Custom LLM Serving** (vLLM, Public Preview) on `GPU_LARGE_RTX` (RTX PRO 6000, about $5.60/h, NVIDIA-validated). The cheaper A10G is untested.
  - The NIM container can't be deployed there, but it is plain vLLM underneath with equal throughput. [dbx-custom-llm] [e4b-tr]
  - **CPU inside the App is not viable for live calls**: about 11 tok/s and 6 s cold on 8 vCPUs *indep.*, while Apps top out at 12 vCPUs. [hackernoon] [doc 07 A2]
- **Local Mac.**
  - Available through Ollama (`nemotron-3.5-bolt:e4b`), GGUF and MLX.
  - Speed on an M4 Pro: about 64 tok/s decode, about 1.05k tok/s prefill, and about 1 s time to first token (TTFT) at a 1K-token prompt *indep.* [omlx-q4]
  - **The untagged Ollama name pulls the 74B model**, which has no Metal support. [ollama] [ollama-tags]

## Model facts

| | **Bolt E4B 4GB** | **Bolt 74B-A9B 24GB** |
|---|---|---|
| Released | 2026-10-23: Hugging Face (HF), build.nvidia.com, Ollama [e4b-card] | 2026-10-15 [b74-card] |
| Lineage | RL (GRPO, RLVR+RLHF) post-train of `gemma-4-E4B-it` [e4b-card] | Compressed from Nemotron 3.5 Super VL 120B-A12B (Star Elastic), then RL and 2-bit quantization-aware distillation (QAD) [b74-card] |
| Parameters | 8B total / 4B effective. Per-layer embeddings (PLE) are lookup tables [e4b-card] [hackernoon] | 74B total / 9B active, Mamba-2/Transformer LatentMoE plus a vision encoder [b74-card] |
| Precision / files | Q4_K_M GGUF (imatrix) is the main release, **6.18 GB on disk**. BF16 checkpoint for vLLM. DFlash drafter (360M) [e4b-tr] [atomic] | IQ2_XXS experts (2.06 bits per weight), NVFP4 Mamba, Q8_0 rest. Packs to 24 GB. GGUF: 22 GB plus a 1.7 GB vision projector. BF16 "for customization, not production" [b74-bf16] [buildmvp] |
| Memory at runtime | 3.1–4.3 GB VRAM on RTX; 4.2–5.0 GB on Jetson *vendor* [e4b-blog]. About 3.8–4.1 GB RAM on CPU *indep.* [hackernoon]. BF16 ≈ 17 GB (Ollama bf16 tag) [ollama-tags] | < 24 GB at batch 1, 128K context *vendor*. KV cache ≈ 4.3 KB/token (Reddit test via [buildmvp]) |
| Context | **64K** in/out [e4b-card]. Ollama's tags say 128K, which conflicts [ollama-tags] | 256K [b74-card] |
| Modalities | Card: **text → text**. But the tech report's NIM accepts image and audio (≤30 s clips; OpenASR WER 9.28), inherited from Gemma [e4b-tr] | Text, image, video → text [b74-card] |
| Languages | Blog: "35 languages" [e4b-blog]. Card: EN/ES/FR/DE/IT/JA "among others". **Tech report: post-trained in English only**. Translation (WMT24++) is on par with Gemma; MMLU-ProX is −1.2 to −1.5 points [e4b-tr] | Primarily English. ES/FR/DE/IT/JA supported [b74-card] |
| Thinking | `chat_template_kwargs.enable_thinking` (**default true**). Budget: `thinking_token_budget` (NIM/vLLM) or `--reasoning-budget` (llama.cpp) [e4b-card] [e4b-tr] | `enable_thinking`, plus `medium_effort` [b74-card] |
| Voice extras | `conversational_mode=true` gives concise, low-formatting replies. Documented async tool-call pattern (a "pending" tool result) [e4b-card] | n/a |
| Tool calling | OpenAI `tools`. vLLM `--tool-call-parser gemma4 --reasoning-parser gemma4`; llama.cpp `--jinja`. **The template rejects stray `tool` messages** [e4b-card] | vLLM `qwen3_coder` parser, plus a custom `ultra_v3` reasoning-parser plugin downloaded from HF [b74-card] |
| Structured output | StructEval-T 80.4 (thinking on) / 78.8 (off) *vendor* [e4b-tr] | "Supports JSON output" [b74-card] |
| Sampling | NVIDIA recommends temp 1.0, top_p 0.95 in both modes [e4b-card] | Same [b74-card] |
| Serving targets | llama.cpp (CUDA/CPU/Metal), vLLM 0.30–0.31 (NIM / BF16 card), SGLang, Ollama, LM Studio, NIM (amd64/arm64). **NIM validated on H100 80GB, RTX PRO 6000/4500 and DGX Spark** [e4b-bf16] [e4b-tr] | vLLM nightly with **b12x kernels (SM120/121 only)**, or llama.cpp. Recipe sets `--max-num-seqs 1` [b74-card] [b12x] |
| License | OpenMDW-1.1. The Gemma 4 base is Apache-2.0 [e4b-card] | OpenMDW-1.1 [b74-card] |

**E4B benchmarks** (BF16, *vendor*, NeMo Gym harness, simulated users and judges are GPT models) [e4b-tr]:

| Benchmark | E4B think-on | E4B **think-off** | Qwen3.5-4B on / **off** | Gemma4-E4B on / off |
|---|---|---|---|---|
| Tau2-Bench (multi-turn customer tasks) | 55.1 | **34.3** | 75.1 / **73.5** | 38.5 / 26.0 |
| BFCL-v3 | 68.5 | 64.7 | 71.0 / 66.0 | 66.4 / 58.1 |
| ToolSandbox | 68.8 | 61.5 | 69.5 / 52.2 | 65.4 / 35.7 |
| When2Call-MT (call, clarify or abstain) | 63.4 | 51.3 | 37.9 / 46.0 | 62.9 / 54.8 |
| IFBench | 63.8 | 53.3 | 58.9 / 34.3 | 42.8 / 38.8 |
| IHEval (instruction hierarchy) | 84.8 | 66.9 | 60.6 / 34.3 | 67.0 / 53.0 |
| MultiChallenge (multi-turn consistency) | 53.1 | 48.8 | 51.8 / 38.6 | 42.1 / 40.7 |
| RoleMRC (persona adherence) | 80.6 | 77.2 | 65.8 / 58.9 | 74.6 / 72.5 |
| RoleBench Custom (persona style; evaluated with thinking off only) | n/a | 67.1 | n/a / 53.7 | n/a / 49.9 |
| Brevity Bench | 82.5 | 78.7 | 75.4 / 75.2 | 80.0 / 75.4 |
| RelyToolBench (tool hallucination) | 63.2 | 61.3 | 43.4 / 19.4 | 61.6 / 58.9 |

- No plain-text IFEval score is published; IFEval appears only inside the audio VoiceBench suite.
- At Q4_K_M, Tau2 drops to 52.3 for E4B and 61.7 for Qwen3.5-4B (thinking on).
- **Independent smoke test** (CPU, Q4, thinking off, 20 prompts × 3 runs, 5 tools) [hackernoon]:
  - Correct tool, or correctly no tool: 60/60.
  - Exact arguments: 57/60. The misses were relative-date arithmetic: "Friday" came out one week late.

**Sibling: Super VL 120B-A12B** (1M context, VLM) [tns]. It is too large for the single ~80 GB GPU that Databricks custom serving allows, and it is reasoning-oriented, so it's out of scope for voice.

## Comparison with the current picks (Qwen3.6-35B-A3B, Gemma 4 26B-A4B)

| | Qwen3.6-35B-A3B (doc 01 pick) | Gemma 4 26B-A4B (doc 01 alternative) | Bolt 74B-A9B 24GB | Bolt E4B |
|---|---|---|---|---|
| Active params | 3B | 3.8B | 9B (2-bit) | 4B effective |
| Tool use, *vendor* (reasoning on, NVIDIA-measured) [b74-tr] | MCP-Atlas 51.2 (Qwen reports 62.8), Tau3 Banking 10.5 | MCP-Atlas 48.4, Tau3 Banking 14.0 | **MCP-Atlas 52.2, Tau3 Banking 17.0** | Tau2 55.1 (on) / **34.3 (off)** |
| Tool use, model vendor's own (doc 01) | TAU3 67.2 | τ2 68.2 | n/a | n/a |
| Instruction following: IFBench *vendor* [b74-tr] [e4b-tr] | 63.7 | **77.3** | 66.2 | 63.8 (on) / 53.3 (off) |
| Non-hallucination (AA-Omniscience) *vendor* [b74-tr] | 65.9 | **14.2** | 61.3 | n/a |
| Thinking-off evals published? | Vendor (doc 01) | Vendor (doc 01) | **No** | **Yes** (all 21 benchmarks) |
| Decode speed, same box: RTX Spark, llama.cpp, batch 1, with drafter [b74-blog] | 96.6 tok/s (MTP) | **100.6 tok/s** (MTP) | 84.0 tok/s (LiLiCorr) | 272.6 tok/s on RTX 5070 (DFlash) [e4b-tr] |
| Agent replay time (AA-AgentPerf Local) [b74-blog] | **454 s** | 481 s | 601 s | n/a |
| TTFT (published) | H100: ≈0.11 s for 3K-token prefill (doc 01) | Same class (doc 01) | Not published | RTX 5070: 548 ms for 3K uncached (llama.cpp Q4) [e4b-blog]. Datacenter: not published |
| Databricks GPU path | Custom LLM Serving on H100 (doc 01). No starter notebook | **Official H100 starter notebook** [dbx-custom-llm] | `GPU_LARGE_RTX` only (SM120); BF16 ≈148 GB (est.) fits no single GPU | Any `GPU_*` ≥ A10G (est.). NIM-validated on H100 / RTX PRO 6000 |
| Local Mac | 4-bit, ~20 GB (doc 01) | Yes (doc 01) | **Ollama: no Metal**. Community MLX conversions exist, unverified [ollama] [hf-api] | Yes (Ollama, GGUF, MLX), 5–8 GB |
| License | Apache-2.0 | Apache-2.0 | OpenMDW-1.1 | OpenMDW-1.1 (+ Apache-2.0 base) |

**Read-out.**
- Bolt 74B beats Qwen and Gemma on NVIDIA's agentic tests: Terminal-Bench, BrowseComp, Tau3 Banking and GDPVal.
- It loses on IFBench, speed and Databricks deployability.
- Our voice workload is short, thinking-off turns with a few tools, which is not what Bolt 74B was tuned for. Community reports also include a tester whose long (>100K-token) tool loops failed 4 of 4 times (anecdotal) [buildmvp].
- **Keep Qwen3.6. Add Bolt 74B as an optional bake-off entrant once the `GPU_LARGE_RTX` spike passes.**

## Fit matrix

| Role | Value | Latency OK? | Quality risk | MVP / later |
|---|---|---|---|---|
| **(a) Main voice LLM: E4B** | Cheap (~4 GB). Strong persona, brevity and IHEval scores | ✅ GPU: ~126 tok/s per user, ~302 with DFlash, on RTX PRO 6000 *vendor* [e4b-tr] | **High.** Tau2 34.3 with thinking off; date arithmetic errors *indep.*; English-only post-training | ❌ Not MVP. Re-test for simple scripted surveys later |
| **(a) Main voice LLM: Bolt 74B** | Best agentic scores in its memory class *vendor* | ⚠️ Slower than Qwen and Gemma [b74-blog]. No datacenter TTFT data. Single-sequence recipe | **High.** No thinking-off evals; 2-bit; SM12x-only kernels; IFBench < Gemma | Later: bake-off entrant only |
| (a) Main voice LLM: Qwen3.6-35B-A3B | Doc 01 pick, unchanged | ✅ (doc 01) | Medium (needs our thinking-off tool eval) | **MVP** |
| **(b) Fast first responder** (E4B says an acknowledgement while the main model or a tool works) | Low–medium. With Qwen at ~0.1–0.3 s TTFT on GPU, it only helps on slow tool turns. Pre-synthesized fillers plus E4B's async "pending tool" pattern cover most of that with one model | ✅ | Medium: the two models can contradict each other or the responder can over-promise ("I've booked it"). Needs a second always-on endpoint | Later, only if measured P95 tool-turn latency is > ~1 s |
| **(c) Local-dev LLM** | High as a **light profile**: 16 GB Macs, CI, offline demos. **Not parity** for the main LLM, which runs Qwen3.6 4-bit locally (doc 01) | ✅ ~64 tok/s, ~1 s TTFT on M4 Pro *indep.* [omlx-q4] | Low for dev; misleading if used to judge main-LLM quality | **MVP** as the second local profile; it also runs the sim caller |
| **(d) Simulated caller** (persona-driven test callers in CI and pre-publish checks) | **High.** Persona adherence (RoleBench 67, RoleMRC 77–81), multi-turn consistency, brevity and `conversational_mode` match "act like a caller". Same weights in prod, local and CI. A different family from the agent (Qwen) reduces self-agreement bias | ✅ Not latency-critical. Batch endpoint can scale to zero. CPU in CI ≈ 2–4 s per caller turn *indep.* | Medium: callers may be too cooperative; persona drift; weaker non-English | **MVP. Best fit** |
| (e) Routing, classification, guardrails | Low. Jev System One gives typed probabilities from one pass at ~$0.042/M input tokens, pay-per-token on FMAPI with no GPU floor (doc 08). E4B could do **free-form slot or JSON extraction** (StructEval-T 78.8 with thinking off), which Jev doesn't do (doc 08) | ✅ | Medium: generative output and uncalibrated scores | Later, for extraction only. MVP uses Jev |
| **(f) Edge / on-prem** | High for regulated or offline customers: 4 GB, runs on RTX/Jetson; NIM image for amd64/arm64; permissive license [e4b-tr] | ✅ on RTX. ❌ Jetson Orin Nano TTFT 5.8 s *vendor* | Same as (a) | Later |

## Databricks serving plan

1. **Not in FMAPI or `system.ai`.**
   - The supported-models page lists no Nemotron model at all on 2026-10-30.
   - The small models it does host are OpenJev (Qwen3.5-4B, decisions only) and Llama 3.1 8B [dbx-models].
   - There's no provisioned-throughput path either.
2. **NIM can't be deployed.**
   - Model Serving takes MLflow-packaged models, not containers [dbx-express].
   - NIM is vLLM 0.30 inside, and NVIDIA measured NIM-to-vLLM throughput at 0.99–1.01× [e4b-tr]. Self-managed vLLM therefore loses nothing.
3. **E4B through Custom LLM Serving** (Public Preview; AWS/Azure US regions; no route optimization, no KV-aware routing, no autoscaling) [dbx-custom-llm]:
   - Use a Standard v6 AI Runtime notebook so we can pin vLLM 0.31.0 (the version the BF16 card targets) [e4b-bf16].
   - Download the BF16 weights to local disk. Log the model with `task: llm/v1/chat` and register it with `env_pack="databricks_model_serving"`.
   - Entrypoint (est.; adapt it from the card's recipe):
     ```
     python -u -m vllm.entrypoints.openai.api_server --model bolt-e4b --served-model-name bolt-e4b \
       --host 0.0.0.0 --port 8080 --max-model-len 16384 --dtype bfloat16 \
       --chat-template bolt-e4b/chat_template.jinja --enable-auto-tool-choice \
       --tool-call-parser gemma4 --reasoning-parser gemma4 \
       --limit-mm-per-prompt '{"image":0,"video":0,"audio":0}'
       # optional: --speculative-config '{"method":"dflash","model":"<drafter dir>","num_speculative_tokens":7}'
     ```
   - The Rust adapter sends `chat_template_kwargs: {enable_thinking: false, conversational_mode: true}` on **every** request, because thinking is on by default.
   - **GPU choice:**

     | Workload type | GPU | DBU/h | ≈ $/h at $0.07/DBU | Status |
     |---|---|---|---|---|
     | `GPU_LARGE_RTX` | RTX PRO 6000 | 80 | $5.60 (~$4.1k/month 24×7) | NVIDIA-validated |
     | `GPU_XLARGE` | H100 | 100 | $7.00 | Validated; us-west-2 only, by enrollment |
     | `GPU_MEDIUM` | A10G (SM86) | 20 | $1.40 | Not in NVIDIA's validated list. BF16 ≈17 GB fits in 24 GB with small KV headroom (est.). The NVFP4 drafter may not help there |

     Sources: [dbx-pricing] [dbx-custom-llm].
   - **Expected performance:**
     - Decode, thinking off, RTX PRO 6000 *vendor*: 126 tok/s per user without the drafter; 302 tok/s at concurrency 1 and 273 tok/s at concurrency 8 with DFlash. DFlash changes TTFT by < 100 ms [e4b-tr].
     - **TTFT for a 2–3K-token prompt ≈ 0.1–0.2 s uncached** (est., by analogy with the 3B-active MoE in doc 01), plus the serving hop. Custom LLM endpoints can't be route-optimized, so they don't get the < 20 ms path in doc 07 §B5.
   - **Concurrency:** dozens of calls per GPU (est.; unmeasured).
   - **Scale-to-zero:**
     - Off for any live-call role.
     - **On for a sim-caller-only endpoint**, where cold starts of minutes are acceptable for CI [dbx-custom-llm].
4. **Bolt 74B (later spike only).**
   - `GPU_LARGE_RTX` is the only fit: the b12x kernels target SM120/121, and the RTX PRO 6000 is compute capability 12.0. H100 is SM90 [b12x] [nv-cc].
   - It needs a vLLM nightly, the b12x/FlashInfer kernels and an HF-downloaded parser plugin.
   - Community AWQ/W4A16 quants (≈40 GB est.) could fit an H100, but they're unverified [hf-api].
5. **CPU inside the Databricks App: no for live calls.**
   - Measured on an 8-vCPU Xeon with AVX-512 *indep.* [hackernoon]:
     - Decode 10.9 tok/s; prefill 195 tok/s.
     - The first request with a ~500-token prompt took 6.4 s.
     - Warm TTFT: 0.36–0.6 s for text, 0.9–1.15 s for tool calls.
     - One stream at a time. About 4 GB of RAM.
   - Apps offer 2, 4 or 12 vCPUs with 6, 12 or 48 GB (doc 07 A2). The CPU architecture and AVX-512 support are unknown (spike S1), and the 6.18 GB GGUF exceeds the 10 MB per-file limit, so it would have to load from a UC Volume at boot.
   - A 2–3K-token agent prompt would take ≥ 10 s cold (est.), and calls would serialize on the CPU.
   - **Acceptable only for low-concurrency sim-caller runs, or as a degraded "LLM down" fallback.**
6. **NVIDIA's hosted API** (`integrate.api.nvidia.com`, model `nvidia/nemotron-3.5-bolt-e4b`) is a free trial endpoint [free-llms]. NVIDIA's free endpoints log usage [openrouter-nv]. **Use it for quick evaluation only, never with call PII.**

## Local-dev plan (Apple Silicon; nothing installed or downloaded for this report)

| Runtime | Command / build | Notes |
|---|---|---|
| **Ollama** | `ollama run nemotron-3.5-bolt:e4b` (Q4_K_M, 7.2–7.9 GB). MLX tags: `:e4b-nvfp4` (7.2 GB), `:e4b-mxfp8` (12 GB), `:e4b-mlx-bf16` (17 GB) | **The untagged `nemotron-3.5-bolt` is the 74B model (25 GB), and it has no Metal support** [ollama] [ollama-tags] |
| **llama.cpp** | `llama-server -hf nvidia/NVIDIA-Nemotron-3.5-Bolt-E4B-4GB:Q4_K_M --jinja -c 16384` | Official GGUF [e4b-card]. Tool-call output streams only once the call is parsed, so tool TTFT ≈ whole-call time *indep.* [hackernoon]. DFlash on Metal is unverified |
| **MLX / LM Studio** | `lmstudio-community/NVIDIA-Nemotron-3.5-Bolt-E4B-MLX-4bit` (or `-MLX-8bit`, `-GGUF`); `mlx-community/...-OptiQ-4bit` | Community builds [hf-api]. Whether `mlx_lm.server` parses Gemma 4 tool calls is unverified, so prefer llama-server or Ollama for tool tests |
| Docker Model Runner | `docker model run hf.co/nvidia/NVIDIA-Nemotron-3.5-Bolt-E4B-4GB:Q4_K_M` | Listed on the HF card [e4b-card]. Runs on the host GPU (doc 01) |

**Measured Mac speed** (all community runs, *indep.*, single sessions):

| Machine | Build | Decode | Prefill / TTFT | Peak memory | Source |
|---|---|---|---|---|---|
| M4 Pro, 16-core GPU, 24 GB | oMLX 4-bit (oQ4e) | 63.9 tok/s | ~1,054 tok/s; 971 ms TTFT at 1K context | 5.0 GB | [omlx-q4] |
| M4 Pro | 6-bit (oQ6e) | 49.7 tok/s | 3.9 s TTFT at 4K context | 6.6 GB | [omlx-q6] |
| Mac mini M4 | llama.cpp Q4_K_M | 26.6 tok/s | n/a | n/a | [llmcheck] |
| M3 Max | MLX 6-bit | ~55 tok/s | n/a | n/a | [llmcheck] |

- **Estimate for an M4 Max** (about 2× the M4 Pro's memory bandwidth): ~90–110 tok/s decode; ~1.5 s cold for a 3K-token prompt; ≤ 0.3 s on a prefix-cache hit (est.).
- **Profiles:**
  - `main`: Qwen3.6 4-bit (~20 GB), per doc 01.
  - `light`: E4B for 16 GB Macs.
  - `sim-caller`: always E4B, ~5 GB, which runs next to `main` on a 36 GB+ Mac.
- All profiles sit behind the same OpenAI-compatible contract.

## Risks and open questions

- **Vendor-only evidence.**
  - Every quality number is NVIDIA's, including its measurements of Qwen and Gemma. NVIDIA itself notes it could not reproduce Qwen3.6's MCP-Atlas score (51.2 vs 62.8 reported) [b74-card].
  - No Artificial Analysis index exists for either model [aa-74b] [free-llms].
  - Independent data so far: one CPU smoke test, community Mac runs and a DGX Spark decode figure [hackernoon] [omlx-q4] [llmcheck].
- **The thinking default is a trap.** Reasoning is on unless disabled per request, and **turning it off costs E4B 21 points on Tau2** [e4b-tr]. Any "budgeted thinking" middle ground needs our own latency-vs-accuracy sweep.
- **The docs contradict each other:**
  - Context: 64K (NVIDIA) vs 128K (Ollama tag).
  - Languages: 35 (blog), 6 listed (card), English-only post-training (tech report).
  - Modalities: text-only (card) vs image and audio (tech report, NIM, Ollama).
  - Treat non-English and audio input as **unproven**.
- **Sampling at temperature 1.0** is NVIDIA's recommendation. Responses vary (one test got "about 900" vs "899.82") *indep.*, and lower temperatures are untested. Put exact numbers and dates into tool results and templates, not into model arithmetic.
- **Chat-template strictness.** Stray `tool` messages are rejected and the turn order must be standard [e4b-card]. The Rust adapter must enforce this, which matters for barge-in-truncated turns.
- **Databricks Custom LLM Serving is Public Preview:**
  - US regions only; no route optimization; no KV-aware routing, so prefix-cache hits fall with more replicas; no autoscaling.
  - GPU capacity is not guaranteed. H100 is enrollment-only in us-west-2. A10G is unvalidated for E4B [dbx-custom-llm].
  - Custom endpoints aren't Unity Gateway destinations, so there's no gateway fallback (doc 07 §B1).
- **Bolt 74B:**
  - SM12x-only kernels, a vLLM nightly pin, and a parser plugin fetched from HF (a supply-chain review item).
  - 2-bit weights; single-sequence reference recipe; anecdotal failures on long tool loops [buildmvp].
- **License (OpenMDW-1.1)** [openmdw] [lf-1.1] [osi]:
  - Allowed: commercial use, modification and redistribution, with no field-of-use or royalty limits. **Outputs are unrestricted**, so SaaS use is fine.
  - Termination: rights end if we sue claiming the model infringes a **patent or copyright**. Version 1.1 widened this from patent-only.
  - **"You are solely responsible for clearing rights of other persons."** The card lists synthetic training data from GPT-5/5.5, Gemini 3 and DeepSeek-V4-Pro [e4b-card]. That is low risk for us, but it should go to legal review once.
  - Redistributing E4B (for example in on-prem bundles) means keeping the OpenMDW and Gemma Apache-2.0 notices.
  - OpenMDW-1.1 is not yet on the SPDX list, and OSI review is pending, so license allowlists may need a custom entry.
- **Open questions (proposed spikes):**
  1. Run our thinking-off tool-calling eval (doc 01) on E4B vs Qwen3.5-4B vs Qwen3.6, using agent scripts with 3–8 tools.
  2. Measure E4B TTFT and throughput on `GPU_LARGE_RTX` vs `GPU_MEDIUM` through Custom LLM Serving.
  3. Run a sim-caller realism study: do E4B personas surface the same agent failures as human testers?
  4. Re-check FMAPI for Nemotron 3.5 additions each quarter.
  5. Check whether Qwen3.5-4B, which has an official A10G starter notebook [dbx-custom-llm] and stronger Tau2, beats E4B for roles (b) and (e).

## Sources

**Primary: NVIDIA**
- [e4b-card] https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Bolt-E4B-4GB
- [e4b-bf16] https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Bolt-E4B-BF16
- [e4b-blog] https://research.nvidia.com/labs/nemotron/NVIDIA-Nemotron-3.5-Bolt-E4B-4GB/
- [e4b-tr] https://research.nvidia.com/labs/nemotron/files/NVIDIA-Nemotron-3.5-Bolt-E4B-4GB-Technical-Report.pdf (Tables 3–5, 8, 11, 12; §9.1)
- [b74-card] https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-24GB
- [b74-bf16] https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-BF16
- [b74-blog] https://research.nvidia.com/labs/nemotron/Nemotron-3.5-Bolt-24GB/
- [b74-tr] https://research.nvidia.com/labs/nemotron/files/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-24GB-Technical-Report.pdf (Tables 7, 8)
- [nv-dev] https://developer.nvidia.com/topics/ai/nemotron
- [nv-cc] https://developer.nvidia.com/cuda/gpus (compute capability table)

**Primary: license**
- [openmdw] https://github.com/OpenMDW/OpenMDW/blob/main/1.1/LICENSE.OpenMDW-1.1
- [lf-1.1] https://www.linuxfoundation.org/press/linux-foundation-releases-openmdw-1.1-nvidia-adopts-openmdw-for-cosmos-isaac-gr00t-ising-and-nemotron-ai-model-families
- [hf-openmdw] https://huggingface.co/blog/linuxfoundation/openmdw
- [osi] https://techstrong.ai/articles/linux-foundation-submits-openmdw-ai-license-to-open-source-initiative/
- SPDX status for 1.1: https://aiwiki.ai/wiki/openmdw (secondary)

**Primary: Databricks**
- [dbx-models] https://docs.databricks.com/aws/en/machine-learning/foundation-model-apis/supported-models (searched for "Nemotron": 0 hits, 2026-10-30)
- [dbx-custom-llm] https://docs.databricks.com/aws/en/machine-learning/model-serving/serve-custom-llms
- [dbx-express] https://docs.databricks.com/aws/en/machine-learning/model-serving/express-deployments
- [dbx-pricing] https://www.databricks.com/product/pricing/model-serving (GPU DBU rates). $/DBU from doc 07

**Runtimes and tooling**
- [ollama] https://ollama.com/library/nemotron-3.5-bolt
- [ollama-tags] https://ollama.com/library/nemotron-3.5-bolt/tags
- [hf-api] https://huggingface.co/api/models?search=Nemotron-3.5-Bolt-E4B and https://huggingface.co/api/models?search=Nemotron-3.5-Bolt-74B (community GGUF/MLX builds)
- [b12x] https://github.com/local-inference-lab/b12x (SM120/SM121 target; moved to FlashInfer)

**Independent or secondary**
- [hackernoon] https://hackernoon.com/running-nvidia-nemotron-35-bolt-e4b-without-a-gpu-benchmark-results (CPU benchmark, single author)
- [omlx-q4] https://omlx.ai/benchmarks/performance/mcgraopl
- [omlx-q6] https://omlx.ai/benchmarks/performance/9mq405ff
- [llmcheck] https://llmcheck.net/models/nemotron-35-bolt-e4b-on-m4/ (aggregates community Mac runs, including a DGX Spark run at 62 tok/s)
- [aa-74b] https://artificialanalysis.ai/models/nemotron-3-5-bolt-74b-a9b (all metrics "N/A", 2026-10-30)
- [tns] https://thenewstack.io/nvidia-nemotron-bolt-laptop/
- [buildmvp] https://www.buildmvpfast.com/blog/nvidia-nemotron-3-5-super-vl-bolt-open-models-2026 (includes Reddit anecdotes)
- [atomic] https://atomic.chat/blog/guides/how-to-run-nemotron-3-5-bolt-e4b-locally
- [free-llms] https://free-llms.ru/nemotron/nemotron-3-5-bolt-e4b (NVIDIA Build endpoint; "not on AA leaderboard")
- [openrouter-nv] https://openrouter.ai/nvidia/nemotron-3.5-lightning:free (NVIDIA free-endpoint logging notice)
- **Unverified lead:** https://www.hpcwire.com/aiwire/2026/10/15/nvidia-expands-nemotron-family-with-highest-efficiency-super-vl-and-bolt-models/ returned HTTP 403, so it is not used.
