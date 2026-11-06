# vak-domain: pure types and rules

## Responsibility

This crate holds the domain model and nothing else:

- entities, value objects and IDs
- the plan graph and its validation
- rule packs and decision types
- lifecycle state machines (agent, version, test run, session)
- `TurnTimeline` and the stage timestamps that metrics are derived from

It is the bottom of the stack, and every other layer speaks its types.

Today (M2 Provision, M3 Plan, M3b Purposes, M4a voice core, M5 Test):

| Module | Holds |
|---|---|
| `ids` | `AgentId`, `AuditEventId`, `TestRunId`, `PurposeId`: UUID v7 wrappers (`from_uuid`, `parse`); `PurposeId::system(use_case)` is the fixed ID of a built-in purpose (`019a0000-0000-7000-8000-00000000000N`) |
| `agent_name`, `use_case`, `jurisdiction` | `AgentName` (1–80 chars, trimmed, no control chars), `UseCase` (six purposes), `Jurisdiction` (`US`, `US-CA`, `UK`…, default `US`) |
| `catalog` | `VOICES` (six Kokoro voices) and `LANGUAGES` (`en-US` default, `en-GB`); `VoiceId`, `Language` parse against them |
| `settings` | `ModelProfile` (`balanced` default, `fast`), `TurnDetection` (200–2000 ms silence, default 500; barge-in on), `AgentSettings` |
| `rule_pack` | `RulePackRef::for_agent` → `<jurisdiction>-<use case>` v1; `RulePack` (built from a `RulePackSpec`: locked texts with the AI disclosure and recording notice required, opt-out phrases and confirmation, forbidden claims, `Retention`); `RulePackCatalog` (each id and version once; `get` the pinned version, `latest`, `validate(plan, pack)`) |
| `plan` | `Plan` of `PlanNode`s (seven `NodeKind`s) and `PlanEdge`s with conditions; shape checks (`PlanShapeError`: IDs, labels, sizes, duplicates); `validate(&RulePack)` → located `PlanError`s (one start, reachable, a path out, non-empty conditions, no dangling paths, the opening starts with the locked texts); `Plan::template(use_case, locked)` |
| `draft` | `Persona`, `AgentDraft` (`new` needs name, use case and voice; pins `PurposeRef::system(use_case)` v1; `apply(DraftPatch)` keeps the rule pack in step, sets the purpose (`PurposeChoice`: its compliance class becomes the use case) and replaces the saved plan or scenarios; `changed_fields`; `content_hash`, with plan lines only when a plan is saved and scenario lines (each scenario's version) only when scenarios are saved, and purpose lines only when the purpose isn't the use case's built-in v1; `scenarios()` the saved set, `scenarios_or_defaults()` the set it is tested with; `with_restored_scenarios` and `with_purpose` after `restore`) |
| `draft_plan` | `DraftPlan::of(draft, packs)`: the saved plan or the use case's template, with the pinned pack's locked texts, opt-out confirmation and validation; `for_purpose(draft, packs, definition)` opens the pinned purpose version's flow instead; `of_flow(purpose, packs)` shows a purpose's starting flow |
| `purpose` | Versioned purposes (`docs/plan/purposes.md`). `Purpose` (a draft plus append-only `PurposeVersion`s; `system`, `duplicate`, `update_draft`, `save_version(&ToolHosts, now)`: read-only, unchanged and invalid drafts refused), `PurposeRef` (id, version), `PurposeDefinition` (name, description, compliance class, persona, starting flow, tools, guardrails, compliance additions; `problems(hosts)`, `content_hash` with `vak.purpose.v1`), `PurposePatch`, `PurposeError`, `PurposeProblem` |
| `purpose::system` | The six built-in purposes, v1 equal to the templates and packs of their use case (`job` sentences, `Plan::template`) |
| `purpose::tools` | `BuiltinTool` (five, `mark_do_not_call` locked on), `HttpTool` (`ToolName` LLM-safe, `ToolInputs` ≤ 10, `HttpMethod`, `ToolUrl` https with a host name only, timeout 100–10,000 ms, `SecretRef` by name), `PurposeTools`, `ToolHosts` (the allow-list: exact hosts and `*.` subdomains). Definitions only: nothing here calls a URL |
| `purpose::guardrails` | `Guardrails` (allowed topics, never-rules, ≤ 5 `EscalationRule`s of a yes/no `when` and `Transfer`, `EndCall { message }` or `Flag`, 1–30 minutes, `PersonalData` never to ask for) |
| `purpose::compliance` | `ComplianceAdditions` (extra opt-out phrases and forbidden claims, shorter recording and transcript retention; no audit field) and `EffectiveCompliance::of(pack, additions)`: tighten-only |
| `purpose::update` | `PurposeUpdate::new(draft, current, next, packs, replace_plan)`: the `PurposeChange`s by area, and `PlanOnUpdate` (`follows`, `replaced`, `kept`): an edited plan is only replaced on request |
| `hash` | `ContentHash` (`sha256:` + 64 lowercase hex), computed with the RustCrypto `sha2` crate |
| `agent` | `Agent`: `create`, `update_draft` (`None` when nothing changes, `AgentError::Archived` when archived), `archive` (once) |
| `progress` | `SetupProgress::derive(draft, facts)`: Provision, Plan, Test, Publish, 25 % each, never stored; `ProgressFacts::from_draft(draft, packs)`: Plan is done when a saved plan validates against the pinned pack; `.with_test_verdict(hash, verdict)`: Test is done when the latest finished run on that hash passed, and only for that exact draft |
| `actor`, `audit` | `Actor` (forwarded user ID and email), `AuditEvent` and `AuditAction` (`agent.created`, `agent.draft_updated`, `agent.archived`, `test_run.started`, `test_run.cancelled`); `PurposeAuditEvent` and `PurposeAuditAction` (`purpose.created`, `purpose.draft_updated`, `purpose.version_saved`) |
| `decision` | System One types: `QuestionId::parse` (lowercase letters, digits and `_`, starting with a letter, 1–64 chars), `Question` (`yes_no`, or `choice` over 1–255 `ChoiceOption`s with unique labels of 1–128 chars), `QuestionKind`, `Answer` (`YesNo { probability }`, `Choice { label, confidence, probabilities }`), `Answer::fits(question)`, `DecisionError` |
| `opt_out` | `OptOutPhrases` (matches whole words, ignoring case and punctuation; "don't" stays one word), `DEFAULT_OPT_OUT_PHRASES` (nine phrases), `DEFAULT_OPT_OUT_CONFIRMATION` and `DEFAULT_OPT_OUT_CHECK_QUESTION` (asked when the decision model is unsure), until rule packs supply them (M4b) |
| `session` | `Speaker` (`caller`, `agent`; `parse`), `OptOutLayer` (`Phrase`, `Decision`, `LlmTool`), `SessionEndReason` (`ClientEnded`, `OptOut(layer)`, `Escalation`, `EndNode`, `Timeout`, `Error`); each has a `vak.media.v1` wire name |
| `scenario` | `ScenarioId` (1–40 lowercase letters, digits, hyphens, starting with a letter), `RubricItem` (a yes/no question of 1–300 characters and the answer it expects), `Scenario::new(ScenarioSpec)` (name 1–80, goal 1–1,000, persona ≤ 1,000, turn cap 1–20, 1–8 rubric items; text trimmed; `version()` a hash of everything that defines it; `caller_instructions()` the simulated caller's prompt), `ScenarioSet` (1–8, unique IDs, at least one required), `ScenarioError` naming each rule. `ScenarioSet::defaults_for(use_case)` (`scenario/defaults.rs`): three per use case, a required caller who opts out mid-call in every set, every rubric question phrased so yes is the good answer. Defaults follow the compliance class of the agent's pinned purpose, which is its use case. **The seam for purposes:** they will supply their own sets here (M5 follow-up) |
| `test_run` | `TestRun`: `queue(draft)` records the draft hash and every scenario, `start(draft, RunModels, now)` re-records them from the draft it actually tests, `record(ScenarioResult)`, `finish` (open scenarios become errors), `cancel` and `interrupt` (open ones skipped), `progress()`, `verdict()` once finished. `Verdict::of`: passed only if every required scenario passed. `ScenarioResult` (`pending`, `running`, `finished(end, transcript, checks, duration)`: passed with no failed check, failed, or an error when the conversation failed or a check got no answer; `skipped`; durations in whole milliseconds). `RubricCheck::score(item, p)`: passes on the expected side of 0.5 (0.5 counts as yes); no answer, or one outside 0–1, fails. `TestRunStatus`, `ScenarioStatus`, `Verdict` with `as_str`/`parse`; `TestRunParts` for `restore` |
| `timeline` | `TurnTimeline`: one reply's stage stamps, as offsets from the session start (the domain reads no clock). `latencies()` gives its `StageLatencies`: STT, LLM time to first token, TTS time to first audio, mouth-to-ear |
| `voice` | `SessionScript::for_agent(draft, definition)`: the locked disclosure ("Hi, this is {name}, an automated AI assistant. This call may be recorded."), LLM instructions from the agent's persona and its purpose version (the persona, then lines for allowed topics, never-rules, never-ask data, the tools it may offer and those it confirms first; never URLs or secret names), the voice, turn detection, the opt-out phrases with the purpose's additions, the confirmation and check question, and the purpose's longest call. `for_draft` uses the use case's built-in purpose |

Contracts: `tests/contract_agent_provisioning.rs` (`agent-provisioning` v1), `tests/contract_plan.rs` (`plan` v1), `tests/contract_test_runs.rs` (`test-runs` v1), `tests/contract_purposes.rs` (`purposes` v1). The packs themselves are TOML in `contracts/rule-packs/`, loaded by `vak-services` (this crate can't parse TOML).

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
- **Content hash:** the canonical encoding (`AgentDraft::canonical_bytes`) is versioned (`vak.agent-draft.v1`) and pinned by tests. Changing it changes every stored hash: bump the version tag. A saved plan appends its lines, and saved scenarios theirs (each scenario's version, `vak.scenario.v1`), so drafts without them keep their hash; so does a purpose other than the use case's built-in v1. A purpose definition has its own encoding (`vak.purpose.v1`).
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
