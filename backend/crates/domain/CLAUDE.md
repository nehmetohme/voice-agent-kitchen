# vak-domain: pure types and rules

## Responsibility

This crate holds the domain model and nothing else:

- entities, value objects and IDs
- the plan graph and its validation
- rule packs and decision types
- lifecycle state machines (agent, version, test run, session)
- `TurnTimeline` and the stage timestamps that metrics are derived from

It is the bottom of the stack, and every other layer speaks its types.

Today (M2 Provision, M3 Plan, M4a voice core):

| Module | Holds |
|---|---|
| `ids` | `AgentId`, `AuditEventId`: UUID v7 wrappers (`from_uuid`, `parse`) |
| `agent_name`, `use_case`, `jurisdiction` | `AgentName` (1–80 chars, trimmed, no control chars), `UseCase` (six purposes), `Jurisdiction` (`US`, `US-CA`, `UK`…, default `US`) |
| `catalog` | `VOICES` (six Kokoro voices) and `LANGUAGES` (`en-US` default, `en-GB`); `VoiceId`, `Language` parse against them |
| `settings` | `ModelProfile` (`balanced` default, `fast`), `TurnDetection` (200–2000 ms silence, default 500; barge-in on), `AgentSettings` |
| `rule_pack` | `RulePackRef::for_agent` → `<jurisdiction>-<use case>` v1; `RulePack` (built from a `RulePackSpec`: locked texts with the AI disclosure and recording notice required, opt-out phrases and confirmation, forbidden claims, `Retention`); `RulePackCatalog` (each id and version once; `get` the pinned version, `latest`, `validate(plan, pack)`) |
| `plan` | `Plan` of `PlanNode`s (seven `NodeKind`s) and `PlanEdge`s with conditions; shape checks (`PlanShapeError`: IDs, labels, sizes, duplicates); `validate(&RulePack)` → located `PlanError`s (one start, reachable, a path out, non-empty conditions, no dangling paths, the opening starts with the locked texts); `Plan::template(use_case, locked)` |
| `draft` | `Persona`, `AgentDraft` (`new` needs name, use case and voice; `apply(DraftPatch)` keeps the rule pack in step and replaces the saved plan; `changed_fields`; `content_hash`, with plan lines only when a plan is saved) |
| `draft_plan` | `DraftPlan::of(draft, packs)`: the saved plan or the use case's template, with the pinned pack's locked texts, opt-out confirmation and validation |
| `hash` | `ContentHash` (`sha256:` + 64 lowercase hex), computed with the RustCrypto `sha2` crate |
| `agent` | `Agent`: `create`, `update_draft` (`None` when nothing changes, `AgentError::Archived` when archived), `archive` (once) |
| `progress` | `SetupProgress::derive(draft, facts)`: Provision, Plan, Test, Publish, 25 % each, never stored; `ProgressFacts::from_draft(draft, packs)`: Plan is done when a saved plan validates against the pinned pack |
| `actor`, `audit` | `Actor` (forwarded user ID and email), `AuditEvent` and `AuditAction` (`agent.created`, `agent.draft_updated`, `agent.archived`) |
| `decision` | System One types: `QuestionId::parse` (lowercase letters, digits and `_`, starting with a letter, 1–64 chars), `Question` (`yes_no`, or `choice` over 1–255 `ChoiceOption`s with unique labels of 1–128 chars), `QuestionKind`, `Answer` (`YesNo { probability }`, `Choice { label, confidence, probabilities }`), `Answer::fits(question)`, `DecisionError` |
| `opt_out` | `OptOutPhrases` (matches whole words, ignoring case and punctuation; "don't" stays one word), `DEFAULT_OPT_OUT_PHRASES` (nine phrases), `DEFAULT_OPT_OUT_CONFIRMATION` and `DEFAULT_OPT_OUT_CHECK_QUESTION` (asked when the decision model is unsure), until rule packs supply them (M4b) |
| `session` | `Speaker` (`caller`, `agent`), `OptOutLayer` (`Phrase`, `Decision`, `LlmTool`), `SessionEndReason` (`ClientEnded`, `OptOut(layer)`, `Escalation`, `EndNode`, `Timeout`, `Error`); each has a `vak.media.v1` wire name |
| `timeline` | `TurnTimeline`: one reply's stage stamps, as offsets from the session start (the domain reads no clock). `latencies()` gives its `StageLatencies`: STT, LLM time to first token, TTS time to first audio, mouth-to-ear |
| `voice` | `SessionScript::for_draft(draft)`: the locked disclosure ("Hi, this is {name}, an automated AI assistant. This call may be recorded."), LLM instructions from the persona and use case, the voice, turn detection, and the default opt-out phrases, confirmation and check question |

Contracts: `tests/contract_agent_provisioning.rs` (`agent-provisioning` v1), `tests/contract_plan.rs` (`plan` v1). The packs themselves are TOML in `contracts/rule-packs/`, loaded by `vak-services` (this crate can't parse TOML).

## Dependencies

- **May depend on:** no workspace crate.
- **External, normal and build:** only `serde` (derive), `thiserror`, `uuid` (v7 IDs), `jiff` (time) and `sha2` (content hash). This is the `external` list in `layers.toml`.
- **External, dev only:** additionally `proptest` and `insta`.
- **Must not depend on:** tokio or any async runtime, sqlx, axum, reqwest, tracing exporters, any adapter, or any other `vak-*` crate.
- `cargo xtask check-layers` fails on anything else.

## Conventions

- **No I/O, no async, no clocks, no randomness.** Callers pass in `now: jiff::Timestamp` and IDs, so every rule is deterministic.
- **Pure rules:** every rule is a pure function or method that returns `Result<T, DomainError>`.
- **Errors:** they are `thiserror` enums of rule violations, with no I/O error types. Name the rule in the variant: `PlanError::UnreachableNode { node }`.
- **Validated types:** make invalid states unrepresentable with newtypes and constructors that validate (`AgentName::parse`). Fields stay private.
- **Liberal input:** parsers trim and case-fold (`UseCase::parse(" Booking ")`, `Jurisdiction::parse("us_ca")`); stored values are canonical.
- **IDs:** typed wrappers over `uuid::Uuid` v7 (`AgentId`, `VersionId`), never bare `Uuid` in signatures.
- **Serde:** DTOs live in `vak-api`, not here. Serde derives exist only for persisted snapshots, such as published versions stored as JSONB.
- **Content hash:** the canonical encoding (`AgentDraft::canonical_bytes`) is versioned (`vak.agent-draft.v1`) and pinned by tests. Changing it changes every stored hash: bump the version tag. A saved plan appends its lines, so drafts without one keep their hash.
- **Panics:** never use `unwrap`, `expect` or `panic!`. Index safely (`get`) and return errors.
- **Visibility:** `pub(crate)` is the default; export only what other layers need.

## Testing

- Unit tests sit next to the code in `#[cfg(test)] mod tests`.
- Property tests (`proptest`) cover state machines, schedule windows, plan validation and round-trips.
- Tests need no fakes, containers or async runtime. If one does, the code is in the wrong layer.

## Coverage floor

**95%** line coverage (`coverage.toml`). Mutation testing (`cargo mutants --in-diff`) runs nightly on this crate as a report.

## Example

```rust
use vak_domain::{AgentDraft, AgentName, ProgressFacts, SetupProgress, UseCase, VoiceId};

let draft = AgentDraft::new(
    AgentName::parse("Front desk")?,
    UseCase::parse("support")?,
    VoiceId::parse("af_heart")?,
);
assert_eq!(draft.rule_pack().id(), "us-support");
assert_eq!(SetupProgress::derive(&draft, &ProgressFacts::default()).percent(), 25);
```
