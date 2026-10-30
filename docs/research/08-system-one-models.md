# 08: TypeSafe "Jev" System One models on Databricks

**Researched 2026-10-30.** Jev launched on 2026-09-15 and Databricks integrated it on 2026-09-30. Almost everything below comes from vendor docs that are less than seven weeks old. The independent evidence is a handful of small studies (§1.3).

Bracketed keys refer to the **Sources** section. **"Inference"** marks our own reading where no source says it outright.

## TL;DR

- **What Jev is.** A *decision model*: it never writes text. You send a `state` plus typed questions and get back probabilities from one parallel pass:
  - `noul`: P(yes).
  - `choice`: picks one of up to 255 labels.
  - `score`: rates on 2–10 ordered levels.

  Price: $0.042 per million input tokens; output is free. The vendor claims 70–500 ms per call. [ts-blog] [ts-models]
- **Independent tests:**
  - **Accuracy:** frontier-level on bounded checks and classification, but last of 12 systems on a LegalBench slice.
  - **Speed:** 3–4× faster than an LLM on a single call; 47× under load and 194× at 32 judgments per call, versus the vendor's 193.6× headline.
  - **Calibration:** good on some tasks, poor on others.
  - **Determinism:** answers vary slightly between identical calls. [vals] [msq] [abdel] [ts-consistency]
- **Databricks (AWS docs).** One TypeSafe-compatible route, `POST /ai-gateway/typesafe/v1/systemone`, serves two models:
  - `system.ai.openjev-qwen35-4b`: hosted by Databricks, Apache-2.0.
  - `system.ai.typesafeai-jev-1-14-0`: hosted by TypeSafe, behind an account preview, under TypeSafe's terms.

  Both cost 0.6 DBU per million input tokens, about $0.042. Separately, `ai_decide` (Beta) covers SQL batch and REST. [dbx-s1] [dbx-models] [dbx-price]
- **Where it fits:**
  - **MVP:**
    - One request per caller turn, with the latency hidden off the critical path. It asks four questions at once (plan-branch routing, intent, opt-out, escalation).
    - Rubric scoring of simulated test calls, plus a gate before publishing.
    - Post-call outcome classification.
  - **Later:** a guardrail on replies, confirmation before tool calls, voicemail detection, `ai_decide` QA and compliance audits, lead scoring.
  - **Never:** writing replies, end-of-turn detection, extracting free-form values, arithmetic.
- **Local dev.**
  - llama.cpp's `llama-server` speaks `/v1/systemone` since 2026-10-02. Serve the Apache-2.0 **Kev-4B** GGUF on it.
  - The wire format is the same, so switching between local and Databricks only changes the URL and model in config.
  - Avoid `openjev/openjev`: it is CC-BY-NC (non-commercial). [llamacpp-pr] [kev]
- **Main risks:**
  - Beta and preview labels everywhere.
  - Databricks' OpenJev has no stated origin and no published benchmarks.
  - The docs contradict each other in places.
  - TypeSafe is a young vendor with one US-West region.
  - Answers vary slightly, and model aliases change underneath you.

  **Mitigations:** the `DecisionEngine` trait; model IDs and thresholds pinned in every published agent version; an offline eval set built from test calls.

---

## 1. What it is

### 1.1 The model class

**Company and launch.** TypeSafe AI is a San Francisco company founded in 2024; CEO Diogo Almeida is a co-inventor of RLHF (ChatGPT). It came out of stealth on 2026-09-15 with a **$40M seed led by DCVC**. [vktr] [reg]

**How it differs from an LLM.** [ts-blog] [ts-s1] [ts-primer]
- **Output:** typed values, not strings. The answers are fixed in advance by the request, so a reply **cannot fail your schema**. That's what TypeSafe's "can't hallucinate" means. It doesn't mean the answer inside the schema is right; TypeSafe concedes its "0%" figure is "not empirical". [ts-blog] [msq]
- **Sampling:** every question is scored **in parallel, in isolation**, against the same state in one pass. Adding questions "barely changes the response time". [ts-intro] [ts-fanout]
- **Training:** "Reinforcement Learning for Calibrated Decisions" (RLCD) optimizes for honest probabilities rather than for answers humans prefer. Calibration holds across groups of answers, "not … an individual answer". [ts-primer] [ts-s1]
- **Not tunable:** Jev is not fine-tuned on customer data; all accounts share the same weights. You adapt it through `state`, `instructions` and `criteria`. [ts-models]
- **Inputs:**
  - Text, JSON objects or arrays. No audio, so we feed it the STT transcript.
  - Image input (`box` questions) is experimental and needs a "Danger-Zone" header. [ts-models] [aisdk]
  - English is the strongest language; test other languages. [ts-models]

**Versions.** Jev 1.14.0 (2026-10-14) and 1.13.0. The aliases `jev-latest` and `jev-preview` both point to 1.14.0 today. They **move** when a release ships, so pin a version if your thresholds were tuned against one. [ts-models]

### 1.2 What "typed probabilistic decisions" look like in practice

A request carries the state and a map of named questions. The response mirrors those names. [ts-api] [dbx-s1] The shapes below are real; the values are illustrative.

```json
{"model":"jev-1.14.0","state":{"last_turns":["Agent: …","Caller: take me off your list"],"node":"pitch"},
 "questions":{"opt_out":{"type":"noul","instructions":"Is the caller asking not to be called again?"},
  "branch":{"type":"choice","instructions":"Which next step fits the caller's last turn?",
            "criteria":{"objection":"Raises an objection","interested":"Wants details","none":null}}}}
→ {"answers":{"opt_out":{"type":"noul","noul":0.97},
   "branch":{"type":"choice","choice":"objection","confidence":0.71,"probabilities":{"objection":0.81,"interested":0.04,"none":0.15}}}}
```

**How code uses the answers.**
- **Thresholds.** Code sets the thresholds, not the model. TypeSafe suggests a three-way split [ts-noul] [ts-conf]:
  - At or above 0.7: act.
  - At or below 0.3: treat it as no.
  - In between: review, or fall back to another path.
- **Confidence.** `confidence` for choices and scores is a fixed formula over `probabilities`. Vals found it "adds no additional signal" beyond them. [ts-conf] [vals]
- **Decompose.** Ask one atomic, gut-check question each, then combine the answers in code (composite scoring). [ts-intro]
- **Known weak spots** in TypeSafe's own list for 1.13 [ts-jagged]:
  - It reads instructions literally.
  - It can't do math, counting or date comparison; keep those in code.
  - It is distracted by large irrelevant state.
  - It can be steered by **adversarial content** (relevant to us: callers can say anything).
  - It has a bias toward the first option in a choice.

### 1.3 Claims versus independent evidence

| Topic | TypeSafe's claim | Independent evidence |
|---|---|---|
| Speed | 70–500 ms; "193.6× faster" [ts-blog] | **Vals AI:** ~0.10 s for 1 judgment, 0.12 s for 64; 47× lower median than GPT-6 Astra on single judgments under load, 194× at 32 judgments [vals]. **themsquared:** p50 379–422 ms from Portland on a home connection, 3.3–3.6× faster than Claude Sonnet 5 [msq]. **AbdelStark:** p50 236–256 ms called from France [abdel] |
| Cost | $0.042/MTok in, free out; "444.6× cheaper" | Vals measured 1/498 of GPT-6 Astra's cost. themsquared measured ~41× cheaper than Sonnet 5 [vals] [msq] |
| Accuracy | "similar intelligence" to frontier LLMs on System One tasks | **Claim verification on SEC filings:** 0.975, tied with GPT-6 Astra. **LegalBench slice:** **last of 12** (class-balanced accuracy 0.730) [vals]. **Tool-call risk:** 91.7%, the same as Sonnet 5 [msq]. **Zero-shot classification:** beats GLiNER2.5 on AG News and Banking77; a tie on emotion [abdel] |
| Calibration | "Calibrated with every output" | **Vals:** ECE 0.011 on claims (best of 12) but 0.108 on LegalBench (worst), so calibration "did not hold" across tasks [vals]. **themsquared:** ECE 0.05–0.07, and every miss came with hedged confidence [msq]. **AbdelStark:** poorly calibrated on emotion (Brier 0.846) [abdel]. **ChaosNLI:** Choice is overconfident where human annotators disagree [chaosnli] |
| Consistency | "more consistent" than LLMs | TypeSafe's own 15-repeat test: mean SD 0.0102, but one answer spanned 0.43–0.53, crossing 0.5 [ts-consistency]. Vals got the same answer on all 5 repeats of 25 items [vals] |
| Voice use | n/a | **Vapi** guardrail experiment: caught "at least three out of four" rule-violating replies, run in parallel with TTS. "Not … deterministic" [vapi] |

**How strong is this evidence?** Each study is small (60–1,500 items) and runs on Jev 1.13. None of them uses phone-call transcripts. Most latency numbers were measured over the public internet to TypeSafe's US-West service, and **none were measured on Databricks**.

---

## 2. The Databricks offering

| | System One route: OpenJev | System One route: Jev 1.14 | `ai_decide` (SQL) | `ai_decide` (REST) | TypeSafe direct |
|---|---|---|---|---|---|
| **Endpoint** | `POST https://<workspace>/ai-gateway/typesafe/v1/systemone` [dbx-s1] | same route | `ai_decide(state, questions [, options])` [dbx-decide] | `POST /api/2.0/ai-functions/ai-decide`, scope `ai-functions` [dbx-decide-api] | `POST https://api.typesafe.ai/v1/systemone`, Bearer key [ts-api] |
| **Model** | `system.ai.openjev-qwen35-4b` (endpoint `databricks-openjev-qwen35-4b`). "Open model based on Qwen3.5" [dbx-models] | `system.ai.typesafeai-jev-1-14-0` [dbx-models] | Chosen by Databricks; "might change the model". Underlying models are Apache-2.0 [dbx-decide] | Same as SQL | `jev-1.14.0`, `jev-1.13.0`, aliases [ts-models] |
| **Hosting and terms** | Hosted by Databricks. Apache-2.0 [dbx-terms] | **Hosted by TypeSafe**, routed through Unity Gateway. TypeSafe's terms and DPA apply, **not** your Databricks agreement [dbx-models] [dbx-terms] | Inside the Databricks perimeter; parameters not stored [dbx-decide] | same | TypeSafe DPA. Zero data retention for enterprise only [ts-legal] |
| **Status** | Pay-per-token; no preview label [dbx-models] | Needs the account-level **"TypeSafe AI Models" preview** [dbx-s1] | **Beta**, launched 2026-09-30 [dbx-blog-decide] [aiaffairs] | **Beta** | Launched as early access with a waitlist [vktr]. One tracker says generally available, no waitlist, since 2026-09-20 [llmref]; unconfirmed |
| **Price** | 0.600 DBU per 1M input tokens, output not priced. At $0.07/DBU (AWS us-east-1 example), that's **$0.042/M** [dbx-price] | 0.600 DBU per 1M input [dbx-price-prop] | Pricing page says "check Databricks SQL pricing"; not itemized [dbx-decide] | not stated | $0.042/M input, output free [ts-models] |
| **Regions (AWS)** | In-region: us-east-1, us-east-2, us-west-2, ca-central-1, sa-east-1. EU/APAC **only with cross-geography routing**. Not in us-west-1 or GovCloud. Pay-per-token only; no provisioned throughput [dbx-mra] | Not on the region page. TypeSafe's service is "based" on the US West Coast [ts-blog] | 13 regions, including eu-central-1, eu-west-1 and eu-west-2. Not eu-west-3, us-west-1 or GovCloud. Not on SQL Classic; DBR ≥ 15.4 [dbx-regions] [dbx-decide] | same as SQL | US West |
| **`noul` answer field** | `noul` | `noul` | **`probability`** | **`probability`** | `noul` |

**Schema and limits.** [dbx-s1] [ts-openapi] [ts-errors] [ts-models]
- **Request:**
  - `model`: the full Unity Catalog name on Databricks.
  - `state`: a string, object or array.
  - `questions`: a non-empty map. Each entry has a `type`, optional `instructions`, and type-specific `criteria`.
- **Response:**
  - `model`: the backend's real model ID, which can differ from the request.
  - `answers`: keyed by question ID, with the `type` tag repeated in each answer.
  - `usage`: `input_tokens`, `output_tokens`.
- **Choice:** 1–255 options, each description optional (`null`). The answer has `choice`, `confidence` and `probabilities` over every option.
- **Score:** ordered levels. The answer has `score` (the probability-weighted mean of level indexes), `legend`, `probabilities` and `confidence`.
  - The **level count is inconsistent across sources**: 1–10 on the System One page and in TypeSafe's OpenAPI; 2–10 in `ai_decide`, the AI SDK and llama.cpp. Use 2–10.
- **Jev context:** 64k tokens per request and 32k per question by default. A header raises both to 262k, but long context is "slower and less accurate".
- **Bodies:** up to 32 MiB, nested at most 64 levels.
- **Errors:** 400, 401, 413, 415, 422 (validation; the body has a `detail` array), 429 and 529.
- **Rate limits:**
  - TypeSafe sets them per organization, in tokens per second and requests per second.
  - **Databricks documents no rate limit, latency target or SLA for this route** (for comparison, `ai_classify` REST allows 1,200 requests/min). [dbx-decide-api]
- **Restriction:** the route accepts only Unity Catalog model services, not "a model provider service or a non-Unity Catalog serving endpoint". [dbx-s1]

**`ai_decide` versus the real-time route: when to use each.**
- **`ai_decide` in SQL** is for **batch over Delta**: post-call QA, compliance audits and re-scoring old calls after a rubric changes.
  - Its questions must be constant across rows.
  - It returns a `VARIANT` with `{response, metadata, error_message}`, so one bad row doesn't fail the query. [dbx-decide]
  - Our call data reaches Delta through the Lakebase sync (report 07).
- **The System One route** is for **in-call and app-side decisions**. Use it there because:
  - You can pin the model, so OpenJev and Jev 1.14 can be compared side by side.
  - The wire format is identical to TypeSafe's and llama.cpp's, so one adapter covers prod and local.
  - The answer types match the `ai_decide` ones (choice, noul, score).
- **`ai_decide` REST** also targets real-time use [dbx-blog-decide], but:
  - Its envelope is different.
  - It renames `noul` to `probability`.
  - Databricks picks the model and may swap it.

  So "directly compatible with the TypeSafe AI API" [dbx-blog-decide] is loose. Skip it for the voice path.

**TypeSafe's own API.**
- Same endpoint shape, plus `GET /v1/models`. [ts-api]
- Official SDKs exist for Python and JavaScript only. [ts-sdk]
- Also available: a Vercel AI SDK provider [aisdk] (its `boolean` maps to `noul`; it reports values rounded to 2 decimals), and an MIT LLM-backed "drop-in" adapter for comparisons. [ts-adapter]
- Using it directly would mean a second vendor contract outside Databricks, so treat it as a benchmark and fallback, not the prod path.

---

## 3. Fit matrix

**Where it sits in the latency budget.** Report 01's budget is about 900 ms mouth-to-ear, with **no slack**. So in-call decisions must stay **off the critical path**:

1. At the onset of VAD silence (report 01 §2), send **one request with every question for this turn**, built on the pre-transcribed text. Re-ask only if the tail of the transcript changes the meaning.
2. Start the LLM speculatively on the current plan node at the same time.
3. If the answer is "switch branch", "opt out" or "escalate" above the threshold, cancel the LLM and re-prompt.

That costs one extra LLM time-to-first-token (~300 ms), and only on the turns that actually branch. A fan-out request costs about 1k tokens, or **~$0.00004 per turn**. Latency from a Databricks App to either model is **unmeasured**; that's spike J1 (§6).

| Use case | Question type | Real-time OK? | Value | MVP / later |
|---|---|---|---|---|
| **Plan branch routing**: which edge leaves the current node, with "stay" as an option | `choice` (edges + `none`), with confidence-gated fallback to an LLM tool call | **Yes**, if p95 is ≤ ~300 ms in-region; hidden inside the end-of-turn and TTFT windows | **High.** Plan transitions become explicit, logged and testable, and the LLM sees only the current node's prompt | **MVP** |
| Intent detection (inbound) | `choice` in the same request | Yes (no extra cost in the fan-out) | High | **MVP** |
| **Opt-out / "stop calling me"** | `noul`, with a **recall-biased** threshold (e.g. ≥ 0.3 starts the opt-out flow) | Yes | **Compliance-critical.** It must be **one of three layers**, alongside a deterministic phrase list and an LLM `mark_do_not_call` tool, plus a post-call `ai_decide` audit. A timeout must never suppress an opt-out | **MVP** |
| Escalation to a human | `noul` ×N (asks for a human, legal threat, distress) + `score` (frustration) | Yes | High | **MVP** |
| Semantic end-of-turn | `noul` | **No.** It runs on every pause; Smart Turn v3 does this locally in 10–65 ms (report 01) | Low | **No** |
| Voicemail / answering-machine detection from the first 2–4 s of transcript | `choice` {human, voicemail, IVR, unknown} | Yes (seconds of slack) | Medium: complements carrier AMD; Vapi floated the same idea [vapi] | Later |
| Guardrail on the agent's reply before it's spoken | one `noul` per rule, per sentence | **Tight.** It must finish within TTS first-audio (~100 ms) or it delays the first clause. Run it in parallel with TTS and gate playout [vapi] | Medium. Vapi caught ≥ 3 of 4 violations, and a false cut-off is audible | Later; run it **offline** first |
| Gating tool calls (did the caller explicitly confirm the booking or payment?) | `noul` | Yes; tool latency is already masked with filler audio | High for irreversible actions | Later (the MVP uses a deterministic confirm step) |
| Plan coverage: required disclosures and goals covered so far | one `score` per item (TypeSafe's "Intelliprompter" pattern) [ts-intelli] | Yes, async | Medium | Later |
| **Scoring simulated test calls against a rubric** | a checklist of `noul`s and `score`s, combined in code | n/a (offline) | **High.** Thousands of rubric checks per run cost cents and are steadier than an LLM judge. Use a different model family from the agent's | **MVP** |
| **Regression gate before publishing** | aggregates of the row above versus the last published version | n/a | **High.** Fits the immutable-publish flow | **MVP** |
| Post-call outcome classification | `choice` (booked, callback, not interested, wrong number, voicemail…) + `noul`s | n/a (seconds) | High: feeds analytics and campaigns | **MVP** (System One route at hang-up) |
| QA rubrics and compliance audits at scale (AI disclosure given? recording notice? opt-out honored?) | `noul`/`score` via **`ai_decide` over Delta** | n/a | High | Later (Beta; needs the Delta sync) |
| Lead scoring and prioritization for campaigns | several `score`s combined by a formula in code | n/a | Medium | Later |

**Where it does not make sense.**
- Generating or rewriting replies; TypeSafe says so itself. [ts-jagged]
- Extracting free-form values such as names, phone numbers, amounts or dates. Find candidates with the LLM or a regex, then let Jev pick one.
- Arithmetic, counting, and comparing dates or times.
- Facts that aren't in the state, such as knowledge-base lookups. Retrieve first.
- Non-English campaigns, until they've been evaluated.
- Acting as the **sole** control for anything legally required.

---

## 4. Rust trait sketch

This follows report 02's conventions:
- Domain types live in `vak-domain` (serde derive only).
- The port lives in `vak-providers`.
- One HTTP adapter covers every System One backend.
- Async traits are native; `dynosaur` provides the `dyn` version for choosing a backend at runtime from config.

```rust
// vak-domain::decision — pure types and rules, no I/O
pub struct QuestionId(pub String);                 // stable key, e.g. "opt_out", "branch"
pub enum Question {
    Noul   { instructions: String, if_true: Option<String>, if_false: Option<String> },
    Choice { instructions: String, options: Vec<(Label, Option<String>)> },   // 1..=255, validated
    Score  { instructions: String, levels: Vec<String> },                     // 2..=10, low→high
}
pub struct DecisionRequest { pub state: DecisionState, pub questions: Vec<(QuestionId, Question)> }
#[derive(serde::Serialize)]                         // the adapter serializes it as the JSON `state`
pub struct DecisionState { pub turns: Vec<TurnText>, pub plan_node: Option<String>, pub facts: Vec<(String, String)> }

pub enum Answer {
    Noul(Prob),                                     // P(yes)
    Choice { label: Label, probs: Vec<(Label, Prob)>, confidence: Prob },
    Score  { expected: f32, probs: Vec<Prob>, confidence: Prob },
}
pub struct DecisionSet { pub model: ModelId, pub answers: Vec<(QuestionId, Answer)>, pub input_tokens: u32 }

/// Typed questions: platform questions use closed Rust enums; user-defined plan branches use `BranchId`.
pub trait TypedQuestion {
    type Out;
    fn question(&self) -> Question;
    fn read(&self, a: &Answer) -> Result<Self::Out, DecisionError>; // checks that the type tag and label exist
}
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallOutcome { Booked, Callback, NotInterested, WrongNumber, Voicemail, Other }

/// Thresholds belong to the *published agent version*, next to the pinned model ID.
pub struct Thresholds { pub act: Prob, pub reject: Prob }   // e.g. 0.7 / 0.3; opt-out: act = 0.3
pub enum Verdict<T> { Act(T), Review(T), Reject }           // three-way split [ts-conf]

// vak-providers::decision — the port
#[dynosaur::dynosaur(pub DynDecisionEngine)]
pub trait DecisionEngine: Send + Sync {
    /// One round trip; all questions share one state. `deadline` is the caller's budget,
    /// e.g. 300 ms in-call or 30 s offline. On timeout, return Err and let callers fail safe.
    async fn decide(&self, req: &DecisionRequest, deadline: std::time::Duration)
        -> Result<DecisionSet, DecisionError>;
    fn model(&self) -> &ModelId;                    // logged on every decision
}
```

**Adapter notes.** The adapter crate is `vak-adapter-decision-systemone`.
- **Wire DTOs** are `#[serde(tag = "type", rename_all = "lowercase")]` enums, mapped to and from the domain types. Two tolerances:
  - Accept `#[serde(alias = "probability")] noul: f64`, so the `ai_decide` naming parses too.
  - Treat unknown answer types (`box`) as an error, never a panic.
- **One `SystemOneHttpEngine { base_url, model, auth }`** covers every backend:
  - **Databricks:** `https://<host>/ai-gateway/typesafe/v1/systemone`. Auth is the app's service-principal OAuth token (report 07). The doc example uses `-u token:…`; whether a Bearer header is accepted needs verifying.
  - **TypeSafe direct:** `https://api.typesafe.ai/v1/systemone`.
  - **Local:** `http://localhost:8080/v1/systemone`.

  That makes local versus prod a config change only, as the brief requires.
- **Other implementations:**
  - `LlmDecisionEngine`: asks our LLM port for constrained JSON with a probability per option (the [ts-adapter] approach). Use it for parity checks, or when no decision model is deployed.
  - `FakeDecisionEngine`: scripted answers, in `vak-testkit`.
- **Validate before sending.** Check option counts, level counts and duplicate labels locally, so a 422 means a bug.
- **Retries.** Retry 429 and 529 with jittered backoff, but **only within the deadline**.
- **Logging.** Log `model`, the question IDs, the probabilities and the latency, but **not** `state` (PII).
- **No crate dependency.** Several unofficial System One crates exist on crates.io, all tiny and new. [crates] Our own client is about 300 lines of `reqwest` + `serde`.

---

## 5. Local-dev plan

| Tier | Backend | Model | License | Use |
|---|---|---|---|---|
| **Default (dev)** | `llama-server` (llama.cpp with PR #29818, merged 2026-10-02) | `ggml-org/Kev-4B-GGUF` Q4_K_M, 3.0 GB [kev-gguf] | Apache-2.0. Base is Qwen3.5-4B-Base, the same family as Databricks' OpenJev [kev] | Same `/v1/systemone` wire format ("TypeSafe-compatible"; no streaming; levels 2–10) [llamacpp-server] |
| CI (CPU) | `llama-server` | `Julia-1` Q8 (0.17 GB, mmBERT-small base) or `Laya` Q8 (0.45 GB) [julia] [laya] | Apache-2.0 | Fast integration tests of the HTTP path and parsing; weak quality, never used for evals |
| Alternative on a Mac | Kev's own server (`python -m kev.serve`); picks MLX automatically on Apple Silicon [kev] | Kev-4B bf16 (8.4 GB of weights) | Apache-2.0 | When llama.cpp's Metal path lags. Python is for dev only |
| Fallback | `LlmDecisionEngine` against the local LLM from report 01 | the dev LLM | n/a | Parity with no decision model installed; slower |
| Unit tests | `FakeDecisionEngine` | n/a | n/a | Deterministic contract tests |

**Mac reality check.**
- **Hardware:** Kev-4B targets "one 24 GB GPU or a 32 GB Apple Silicon Mac". On an M5 with MLX, an 8k-token state takes 6.6 s fresh and 354 ms cached. [kev]
- **Keep in-call state short** (the last 2–4 turns plus the current plan node). That's also what Jev's docs recommend for accuracy. [ts-jagged] Short-state latency on a Mac isn't published, so measure it (spike J3).
- **Docker:** our assumption, to verify, is that Docker containers on macOS run CPU-only, without Metal. So run `llama-server` natively, with Compose pointing at `host.docker.internal`.
- **Quality gap:** Kev-4B **trails Jev** on held-out data (0.690 versus 0.757 on breadth-v1; 0.565 versus 0.840 on MMLU-Pro; author-reported). [kev] Local is fine for development, but thresholds come from evals against the prod model.

**Avoid:**
- `openjev/openjev` and its GGUF and MLX conversions (~27B parameters, **CC-BY-NC-4.0**, non-commercial), despite their strong self-reported results. [openjev-hf] [openjev-gguf]
- `AlexWortega/openjev` (a different premise/hypothesis NLI interface [alexw]), and the many other "OpenJev" repos on Hugging Face [hf-search]. Those have unclear licenses, no evals, and no TypeSafe affiliation. [semif]

---

## 6. Risks and open questions

| Risk | Mitigation |
|---|---|
| **Beta and preview everywhere.** `ai_decide` is Beta; Jev 1.14 is an account preview; Databricks may swap `ai_decide`'s model [dbx-decide] | Voice path uses the System One route with a **pinned** model ID; `ai_decide` only for analytics |
| **Databricks' OpenJev 4B has no stated provenance or benchmarks.** Expect it to be weaker than Jev 1.14 (inference, based on how open 4B models compare [kev]) | Spike J2: run the eval set on both. Jev 1.14 is opt-in after a DPA review, because its data leaves Databricks for TypeSafe [dbx-terms] |
| **Thin and inconsistent docs**: `noul` vs `probability`; 1–10 vs 2–10 levels; no latency or rate limits for the route; basic-auth example; Jev 1.14 image support listed differently by different sources [dbx-models] [ts-models] | Tolerant serde; local validation; contract tests against each backend; ask the Databricks account team |
| **New vendor.** Founded 2024, one US-West region; on pricing: "We can't prove it isn't subsidized" [ts-blog] | The trait plus an open wire format, implemented by llama.cpp, Kev, the AI SDK and others, keeps **lock-in low**. Only the `ai_decide` SQL is Databricks-specific |
| **Output variance and alias drift.** Answers near a threshold flip between runs [ts-consistency]; `jev-latest` moves [ts-models] | Review bands rather than single cut-offs; pin `jev-1.14.0` (never an alias); store the model ID and thresholds in each published agent version; re-run evals before changing them |
| **Calibration doesn't transfer across tasks** [vals]; overconfident on ambiguous items [chaosnli]; callers can inject adversarial text [ts-jagged] | Calibrate thresholds **per question family** on our own data; include adversarial "ignore your instructions…" turns in the eval set |
| **Compliance.** A probabilistic opt-out detector can miss | Layered opt-out (§3); fail safe on timeout; a post-call audit of 100% of calls; the compliance report sets the rules |
| **Latency on Databricks unknown**; Jev 1.14 adds a Unity Gateway hop to US-West | Spike J1 before committing in-call uses; keep decisions off the critical path |
| **EU data residency.** OpenJev runs in EU regions only via cross-geography routing [dbx-mra] | Use US workspaces for the MVP; revisit before EU campaigns |

**Evaluation plan** (an offline eval set built from test calls):
1. **Build the set.** Take turns from simulated and real test calls. Label ≥ 300 items per question family: opt-out, escalation, the routing edges of 2–3 reference plans, outcomes, rubric items.
   - Deliberately include ambiguous, adversarial and non-English slices. Sets of only easy items hide calibration problems. [msq]
2. **Measure:**
   - accuracy and macro-F1;
   - Brier score, and ECE with bin occupancy;
   - **coverage at a fixed error budget**, with thresholds frozen on a 30% calibration split (Vals' method [vals]);
   - **recall at the opt-out threshold** (target ≥ 99% for the layered system);
   - stability over 5 repeats, and with shuffled option order;
   - p50 and p95 latency from a Databricks App in the same region.
3. **Compare four backends:** OpenJev on Databricks, Jev 1.14 via Databricks, local Kev-4B, and an LLM judge as baseline. Re-run as a CI job whenever a model ID or a rubric changes.

**Open questions and spikes.**
- **J1:** Latency (p50/p95) and rate limits of the System One route from an App, for each model. Does it accept Bearer auth?
- **J2:** Which checkpoint and recipe is "OpenJev (Qwen3.5 4B)", and how good is it on our eval set?
- **J3:** Short-state latency on a Mac for Kev-4B through llama.cpp (Metal) versus MLX.
- How is `ai_decide` billed, and which model powers it? Is a REST rate limit set?
- Can the route serve **our own** Unity Catalog model service, for example a fine-tuned Kev? Databricks' open-Jev blog uses `ai_query` with a custom endpoint instead. [dbx-blog-openjev]
- Where does Jev 1.14 run, and what is its data-residency story when reached through Unity Gateway?

---

## Sources

- [ts-blog] TypeSafe, "Introducing System One Models & Jev": https://typesafe.ai/blog/introducing-system-one-models-and-jev
- [ts-intro] https://docs.typesafe.ai/introduction · [ts-s1] https://docs.typesafe.ai/concepts/system-one · [ts-primer] https://docs.typesafe.ai/introduction/machine-learning-primer
- [ts-models] https://docs.typesafe.ai/models (table in https://docs.typesafe.ai/models.md)
- [ts-api] https://docs.typesafe.ai/api/system-one · [ts-openapi] https://api.typesafe.ai/openapi.json · [ts-errors] https://docs.typesafe.ai/api/errors · [ts-sdk] https://docs.typesafe.ai/sdk
- [ts-conf] https://docs.typesafe.ai/confidence · [ts-noul] https://docs.typesafe.ai/primitives/noul · [ts-fanout] https://docs.typesafe.ai/patterns/fan-out
- [ts-jagged] https://docs.typesafe.ai/model-jaggedness/jev-1.13 · [ts-consistency] https://docs.typesafe.ai/cookbooks/consistency_noul_cookbook · [ts-intelli] https://docs.typesafe.ai/cookbooks/intelliprompter
- [ts-legal] https://docs.typesafe.ai/legal · [ts-adapter] https://github.com/typesafe-ai/system-one-adapter-python
- [vktr] https://www.vktr.com/ai-platforms/chatgpt-cocreator-launches-typesafe-ai-with-jev/
- [reg] https://www.theregister.com/ai-and-ml/2026/09/16/typesafe-ai-debuts-model-for-machines-that-plays-doom/5296711
- [aisdk] https://ai-sdk.dev/providers/ai-sdk-providers/typesafe-ai · [llmref] https://www.llmreference.com/provider/typesafe-ai
- [dbx-s1] https://docs.databricks.com/aws/en/machine-learning/model-serving/query-systemone-models (Azure: https://learn.microsoft.com/en-sg/azure/databricks/machine-learning/model-serving/query-systemone-models)
- [dbx-models] https://docs.databricks.com/aws/en/machine-learning/foundation-model-apis/supported-models · [dbx-terms] https://docs.databricks.com/aws/en/machine-learning/model-serving/acceptable-use-models
- [dbx-mra] https://docs.databricks.com/aws/en/unity-gateway/model-region-availability · [dbx-regions] https://docs.databricks.com/aws/en/resources/feature-region-support
- [dbx-decide] https://docs.databricks.com/aws/en/sql/language-manual/functions/ai_decide · [dbx-decide-api] https://docs.databricks.com/api/ai-functions/v1/ai-decide
- [dbx-price] https://www.databricks.com/product/pricing/foundation-model-serving · [dbx-price-prop] https://www.databricks.com/product/pricing/proprietary-foundation-model-serving
- [dbx-blog-decide] https://www.databricks.com/blog/introducing-aidecide-make-fast-decisions-your-governed-data · [dbx-blog-openjev] https://www.databricks.com/blog/running-open-jev-sql-databricks
- [aiaffairs] https://www.aiaffairs.com/technology/databricks-launches-ai-decide-governed-data/
- [vals] Vals AI, 2026-10-06: https://www.vals.ai/blogs/independent-evaluation-of-jev
- [msq] https://github.com/themsquared/jev-benchmark · [abdel] https://github.com/AbdelStark/jev-benchmarks
- [chaosnli] https://abdelstark.github.io/awesome-typesafe-jev/projects/gh-gautamtalksdev-jevbench/ (a listing; paper at https://zenodo.org/records/22971492)
- [vapi] https://vapi.ai/blog/early-experiment-with-jev
- [llamacpp-pr] https://github.com/ggml-org/llama.cpp/pull/29818 · [llamacpp-server] https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md
- [kev] https://huggingface.co/jaredpalmer/kev-4b (repo https://github.com/jaredpalmer/kev) · [kev-gguf] https://huggingface.co/ggml-org/Kev-4B-GGUF
- [julia] https://huggingface.co/ggml-org/Julia-1-GGUF · [laya] https://huggingface.co/ggml-org/Laya-GGUF
- [openjev-hf] https://huggingface.co/openjev/openjev · [openjev-gguf] https://huggingface.co/ggml-org/OpenJev-GGUF · [alexw] https://huggingface.co/AlexWortega/openjev
- [semif] https://github.com/TheoLeeCJ/SemIf-OpenJev · [hf-search] https://huggingface.co/models?search=openjev · [crates] https://crates.io/search?q=systemone
