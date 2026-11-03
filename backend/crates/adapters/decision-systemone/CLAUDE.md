# vak-adapter-decision-systemone

Read `../CLAUDE.md` first; the shared adapter rules apply here.

## Responsibility

This crate implements `DecisionEngine` (from `vak-providers`) as `SystemOneDecisions`, over the System One API. The API returns typed, fast decisions instead of generated text. Each request carries `model`, a `state` with the turns as `{speaker, text}` (plus `plan_node` when set), and `questions` keyed by question ID. All questions are answered in one round trip.

Question types today (from `vak_domain::QuestionKind`):

- **Yes/no** (`noul`): opt-out today; escalation later. The answer's `noul` (or `ai_decide`'s `probability`) becomes `Answer::YesNo`.
- **Choice** (`choice`, with `criteria`: label to description or null): the branch over a plan node's outgoing edges (M4b), intent. It becomes `Answer::Choice` with its label, confidence (or the chosen label's probability) and every option's probability.
- **Scores and outcomes** are later; the domain has no score type yet.

Every answer must fit its question (`Answer::fits`: the same type, probabilities from 0 to 1, a known label with exactly one probability per option). A missing or ill-fitting answer is `ProviderError::Protocol`. Requests the API would refuse (no questions, a repeated ID) fail before any traffic.

- **Config:** `SystemOneConfig::new(base_url, model)` posts to `{base_url}/v1/systemone` (llama-server). `with_path` sets another path under the base URL, such as Databricks' `/ai-gateway/typesafe/v1/systemone` (`VAK_DECISION_PATH`). It also takes `with_bearer_token` and `with_connect_timeout`. An `https://` base URL is refused with `BuildError::BaseUrl` until M7.
- **Errors:** 401 and 403 are `Auth`; 429 and 529 (overloaded) are `RateLimited`; 504 is `Timeout`; any other 5xx is `Unavailable`; anything else is `Protocol`.

The model is `openjev-qwen35-4b`, and Kev-4B serves locally. The pipeline asks the platform's `opt_out` question about every caller turn (`vak-pipeline` `src/routing.rs`; see `contracts/decisions/README.md`).

## Targets

| Environment | Endpoint | Auth |
|---|---|---|
| Production | Databricks Unity Gateway, System One API, `/ai-gateway/typesafe/v1/systemone` | OAuth M2M token from `vak-databricks-auth` (M7) |
| Local dev | llama-server `:8081`, `/v1/systemone` (Kev-4B) | none |

The two targets share one code path; base URL, path and auth come from config.

## Dependencies

- **May depend on:** `vak-providers`, `vak-domain` (decision types), `vak-audio`, `vak-databricks-auth`. Dev-only: `vak-testkit`.
- **Uses today:** `vak-providers`, `vak-domain`, plus `reqwest` (`json`), `serde_json`, `secrecy`, `thiserror`, `tracing` and `tokio` (`time`). Dev-only: `wiremock`.
- **Must not depend on:** other adapters, repositories(-pg), services, pipeline, api.

## Notes

- **Decisions are in the voice path.** Use tight per-call deadlines. The pipeline gives the opt-out question 2 s by default and holds a reply for its answer for at most 300 ms.
- **Return confidence with every answer.** The pipeline applies thresholds and fallbacks. A timeout is a `ProviderError::Timeout`, never a "no" answer, because a timeout must never suppress an opt-out.
- **Transcripts in the request are PII.** Never log them. The debug log line records only the question count, the elapsed time and the outcome.
- **Tests:** `tests/contract_voice_core_decisions.rs` (`voice-core` v1) runs `decision_conformance` against wiremock. Its ignored live test runs the suite against `VAK_DECISION_BASE_URL` with `VAK_DECISION_MODEL`, and `VAK_DECISION_PATH` when set (`just adapters-live`, with `just dev` running).

## Coverage floor

**75%**.
