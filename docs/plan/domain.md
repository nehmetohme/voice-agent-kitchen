# Domain model, data and API

## 1. Core concepts

| Concept | What it is | Notes |
|---|---|---|
| **Agent** | A voice agent the operator owns | Has one mutable **draft** and many immutable **versions** |
| **Draft** | The agent's current editable configuration | Autosaved; has a content hash |
| **Persona** | Name, purpose, tone, language, voice | Create needs only name, purpose and voice; everything else has defaults (Tesler's law) |
| **Plan** | Goals, a flow graph, knowledge, guardrails, escalation rules | Validated by pure domain rules |
| **Flow graph** | Nodes (`start`, `say`, `ask`, `collect`, `tool`, `transfer`, `end`) joined by edges | Each edge has a plain-language condition; the decision model chooses among a node's outgoing edges |
| **Use case** | `support`, `service`, `booking`, `survey`, `telemarketing`, `custom` | Chosen at creation; selects a template and a default rule pack. Since M3b it is a purpose's compliance class |
| **Purpose** | A versioned, workspace-wide definition of what agents are for: name, description, compliance class, persona, starting flow, tools, guardrails and compliance additions (`docs/plan/purposes.md`) | Six built-in purposes (one per use case, read-only, v1 equal to the templates and packs); agents pin a purpose version |
| **Tool** | A built-in action (`transfer_to_human`, `end_call`, `record_answer`, `schedule_callback`, `mark_do_not_call`, always on) or a custom HTTP tool on the allow-list | Definitions only until M4b, which runs them |
| **Guardrails** | Allowed topics, never-rules, escalation rules, limits (longest call, personal data never to ask for) | Topics, never-rules and never-ask are in the LLM instructions of test calls and test runs; enforcement comes with M4b |
| **Jurisdiction** | e.g. `US`, `US-CA`, `EU-FR`, `UK` | Per agent; selects rule-pack variants |
| **Rule pack** | Versioned compliance configuration: locked disclosure text, forbidden claims, opt-out phrases, recording notice, retention, and later calling windows and consent rules | Data, not code paths. Agents pin a rule-pack version |
| **Version** | Immutable snapshot of a draft, with its content hash | Only created by publishing |
| **Publication** | Which version is live, per channel | MVP channel: browser preview link. Phone and widget later. Rollback moves the pointer |
| **Session** | One conversation: live browser test or simulated test | Later also phone inbound and outbound |
| **Turn** | One utterance by the caller or the agent | Stores text, plan node, per-stage timings and decisions |
| **Decision** | A typed answer from the decision engine (branch, intent, opt-out, escalation, outcome) | Stores model ID, probabilities and threshold verdict. State text is not logged (PII) |
| **Scenario** | A scripted test: simulated-caller persona and goal, a required flag, a turn cap, plus a rubric | Part of the draft (the use case's defaults until saved). Rubric items are `noul` yes/no questions with the answer they expect; scores come later |
| **Test run** | Running a set of scenarios against one draft hash | A job; a status per scenario, and one verdict: passed only if every required scenario passed |
| **Audit event** | Append-only record of who did what and when | Kept for 5 years by default (rule pack) |

## 2. Lifecycles

### Agent setup progress (Zeigarnik effect)

```
Provisioned ─► Planned ─► Tested ─► Published
  required       plan        passing test run      live version
  fields set     validates   on the current        = current
                             draft hash            draft hash
```

Progress is derived from state, never stored, so it can't drift. Editing the draft after a test clears the "Tested" step until a new run passes.

### Publish gate

A draft can be published only if all three of these hold:

1. The plan validates.
2. The latest test run on **the exact draft hash** passed every required scenario.
3. Rubric scores are not worse than those of the currently published version, within a set tolerance (regression gate).

Publishing creates the version, moves the publication pointer, and writes an audit event in one transaction. Rollback points the publication at an earlier version, which also writes an audit event.

### Session

```
Connecting ─► Greeting (locked disclosure) ─► Conversing ⇄ Agent speaking
                                                  │ (barge-in returns to Conversing)
                     opt-out / escalation / end node / timeout ─► Ending ─► Ended (outcome classified)
```

## 3. Data (Postgres, sqlx migrations)

| Table | Key columns | Notes |
|---|---|---|
| `workspaces` | id, name | One workspace in MVP; the column exists for multi-tenancy later |
| `operators` | id, workspace_id, external_id, email | From Databricks SSO headers |
| `agents` | id, workspace_id, name, use_case, jurisdiction, archived_at | |
| `agent_drafts` | agent_id (PK), persona JSONB, plan JSONB, scenarios JSONB, settings JSONB, rule_pack_id, rule_pack_version, purpose_id, purpose_version, content_hash, updated_at, updated_by | JSONB validated by domain types on every read and write; `scenarios` is NULL until saved (M5); since M3b every draft pins a purpose version (built-in purposes have fixed IDs) |
| `purposes` | id, workspace_id, name, draft JSONB, draft_hash, latest_version, revision, created_at, created_by, updated_at | The workspace's own purposes (M3b); built-in purposes live in code |
| `purpose_versions` | purpose_id, version, definition JSONB, content_hash, created_at, created_by | Append-only (trigger) |
| `agent_versions` | id, agent_id, number, snapshot JSONB, content_hash, test_run_id, created_by, created_at | Append-only; a DB trigger forbids UPDATE and DELETE |
| `publications` | agent_id, channel, version_id, updated_at | One row per agent and channel |
| `rule_packs` | id, version, jurisdiction, use_case, body JSONB | Later. Since M3 the packs are `contracts/rule-packs/*.toml`, embedded in the server (`vak-services`); immutable per version |
| `templates` | id, use_case, body JSONB | Later. Since M3 the starting plan per use case is domain code (`Plan::template`); the draft stores a plan only once it is saved (`agent_drafts.plan`) |
| `test_runs` | id, agent_id, created_by, draft_hash, status, verdict, llm_model, decision_model, seed, scenarios JSONB, revision, created_at, started_at, finished_at | M5. Each scenario's definition, status, transcript and rubric checks live in `scenarios`; one queued or running run per agent (a partial unique index). Scenarios themselves are part of the draft (`agent_drafts.scenarios`), not a table |
| `sessions` | id, agent_id, version_id or draft_hash, mode, started_at, ended_at, outcome, metrics JSONB | |
| `turns` | session_id, idx, speaker, text, plan_node, timings JSONB, decisions JSONB | Retention per rule pack |
| `audit_events` | id, at, actor, action, entity, entity_id, details JSONB | Append-only (trigger) |
| `jobs` | apalis-postgres tables | Later. Since M5 test runs are their own job rows (`test_runs.status = 'queued'`), claimed by an in-process worker |

Later: `contacts`, `consents`, `dnc_entries`, `campaigns`, `call_attempts`, `phone_numbers`.

## 4. API surface (`/api/v1`)

The REST API is documented in `contracts/openapi.json`. Errors follow RFC 9457 (`application/problem+json`).

| Area | Endpoints |
|---|---|
| Health | `GET /health`, `GET /ready` |
| Identity | `GET /me` |
| Catalog | `GET /catalog/voices`, `GET /catalog/languages`, `GET /catalog/tools`, `GET /templates`, `GET /rule-packs` |
| Agents | `POST /agents` (3 required fields), `GET /agents`, `GET /agents/{id}`, `PATCH /agents/{id}` (autosave), `DELETE /agents/{id}` (archive), `GET /agents/{id}/progress` |
| Plan | `GET /agents/{id}/plan`, `PUT /agents/{id}/plan`, `POST /agents/{id}/plan/validate` |
| Purposes | `GET /purposes`, `POST /purposes` (duplicate), `GET /purposes/{id}`, `PATCH /purposes/{id}` (autosave), `GET`/`PUT /purposes/{id}/flow`, `POST /purposes/{id}/versions`, `GET /agents/{id}/purpose`, `POST /agents/{id}/purpose/update` |
| Live test | `POST /agents/{id}/sessions` returns a session ID and media URL; `GET /sessions/{id}`; `GET /sessions/{id}/turns` |
| Voice | `GET /voice/latency`: per-stage latency histograms (STT, the opt-out decision, LLM time to first token, TTS first audio, mouth-to-ear) with p50 and p95 |
| Scenarios | `GET`, `PUT /agents/{id}/scenarios` (the whole set; the use case's defaults until saved) |
| Test runs | `POST /test-runs` (`{agentId}`, 202), `GET /test-runs/{id}`, `POST /test-runs/{id}/cancel`, `GET /agents/{id}/test-runs` |
| Publish | `GET /agents/{id}/versions`, `POST /agents/{id}/versions` (publish), `POST /agents/{id}/rollback`, `GET /agents/{id}/publications` |
| Audit | `GET /audit?entity=&id=` |

### Media WebSocket: `GET /api/v1/sessions/{id}/media`

- Subprotocol `vak.media.v1`. The upgrade must offer it. A session's socket opens once, within a minute of `POST /agents/{id}/sessions`, for the operator who started it.
- **Binary frames** carry audio in 20 ms frames, PCM s16le mono: 16 kHz from the client, 24 kHz from the server (Kokoro's native rate, so nothing resamples). Opus later.
- **Text frames** carry JSON events, each with a `type`:
  - from the client: `session.start`, `client.mute`, `session.end`
  - from the server: `session.started`, `transcript.partial`, `transcript.final`, `agent.audio.start`, `agent.audio.end`, `agent.interrupted`, `plan.node`, `decision`, `latency`, `error`, `session.ended`
- The server sends nothing before `session.start`. It then answers `session.started` and speaks the locked disclosure as turn 0. `session.ended` is the last event, and on an opt-out it names the layer: `phrase`, `decision` or `llm_tool`.
- Event schemas live in `contracts/media/vak.media.v1.json` (with `examples.json`) and are checked by contract tests on both sides.
