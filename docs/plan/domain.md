# Domain model, data and API

## 1. Core concepts

| Concept | What it is | Notes |
|---|---|---|
| **Agent** | A voice agent the operator owns | Has one mutable **draft** and many immutable **versions** |
| **Draft** | The agent's current editable configuration | Autosaved; has a content hash |
| **Persona** | Name, purpose, tone, language, voice | Create needs only name, purpose and voice; everything else has defaults (Tesler's law) |
| **Plan** | Goals, a flow graph, knowledge, guardrails, escalation rules | Validated by pure domain rules |
| **Flow graph** | Nodes (`start`, `say`, `ask`, `collect`, `tool`, `transfer`, `end`) joined by edges | Each edge has a plain-language condition; the decision model chooses among a node's outgoing edges |
| **Use case** | `support`, `service`, `booking`, `survey`, `telemarketing`, `custom` | Chosen at creation; selects a template and a default rule pack |
| **Jurisdiction** | e.g. `US`, `US-CA`, `EU-FR`, `UK` | Per agent; selects rule-pack variants |
| **Rule pack** | Versioned compliance configuration: locked disclosure text, forbidden claims, opt-out phrases, recording notice, retention, and later calling windows and consent rules | Data, not code paths. Agents pin a rule-pack version |
| **Version** | Immutable snapshot of a draft, with its content hash | Only created by publishing |
| **Publication** | Which version is live, per channel | MVP channel: browser preview link. Phone and widget later. Rollback moves the pointer |
| **Session** | One conversation: live browser test or simulated test | Later also phone inbound and outbound |
| **Turn** | One utterance by the caller or the agent | Stores text, plan node, per-stage timings and decisions |
| **Decision** | A typed answer from the decision engine (branch, intent, opt-out, escalation, outcome) | Stores model ID, probabilities and threshold verdict. State text is not logged (PII) |
| **Scenario** | A scripted test: simulated-caller persona and goal, plus a rubric | Rubric items are `noul`/`score` questions with thresholds |
| **Test run** | Running a set of scenarios against one draft hash | Produces a verdict per scenario and overall |
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
| `agent_drafts` | agent_id (PK), persona JSONB, plan JSONB, settings JSONB, rule_pack_id, rule_pack_version, content_hash, updated_at, updated_by | JSONB validated by domain types on every read and write |
| `agent_versions` | id, agent_id, number, snapshot JSONB, content_hash, test_run_id, created_by, created_at | Append-only; a DB trigger forbids UPDATE and DELETE |
| `publications` | agent_id, channel, version_id, updated_at | One row per agent and channel |
| `rule_packs` | id, version, jurisdiction, use_case, body JSONB | Seeded from `contracts/rule-packs/*.toml`; immutable per version |
| `templates` | id, use_case, body JSONB | Seeded; starting plans per use case |
| `test_scenarios` | id, agent_id, name, persona JSONB, rubric JSONB, required | |
| `test_runs` | id, agent_id, draft_hash, status, verdict, started_at, finished_at | |
| `test_results` | test_run_id, scenario_id, session_id, scores JSONB, passed | |
| `sessions` | id, agent_id, version_id or draft_hash, mode, started_at, ended_at, outcome, metrics JSONB | |
| `turns` | session_id, idx, speaker, text, plan_node, timings JSONB, decisions JSONB | Retention per rule pack |
| `audit_events` | id, at, actor, action, entity, entity_id, details JSONB | Append-only (trigger) |
| `jobs` | apalis-postgres tables | Test runs and retention sweeps run as jobs |

Later: `contacts`, `consents`, `dnc_entries`, `campaigns`, `call_attempts`, `phone_numbers`.

## 4. API surface (`/api/v1`)

The REST API is documented in `contracts/openapi.json`. Errors follow RFC 9457 (`application/problem+json`).

| Area | Endpoints |
|---|---|
| Health | `GET /health`, `GET /ready` |
| Identity | `GET /me` |
| Catalog | `GET /catalog/voices`, `GET /catalog/languages`, `GET /templates`, `GET /rule-packs` |
| Agents | `POST /agents` (3 required fields), `GET /agents`, `GET /agents/{id}`, `PATCH /agents/{id}` (autosave), `DELETE /agents/{id}` (archive), `GET /agents/{id}/progress` |
| Plan | `GET /agents/{id}/plan`, `PUT /agents/{id}/plan`, `POST /agents/{id}/plan/validate` |
| Live test | `POST /agents/{id}/sessions` returns a session ID and media URL; `GET /sessions/{id}`; `GET /sessions/{id}/turns` |
| Scenarios | `GET/POST /agents/{id}/scenarios`, `PUT/DELETE /scenarios/{id}` |
| Test runs | `POST /agents/{id}/test-runs`, `GET /test-runs/{id}` |
| Publish | `GET /agents/{id}/versions`, `POST /agents/{id}/versions` (publish), `POST /agents/{id}/rollback`, `GET /agents/{id}/publications` |
| Audit | `GET /audit?entity=&id=` |

### Media WebSocket: `GET /api/v1/sessions/{id}/media`

- Subprotocol `vak.media.v1`.
- **Binary frames** carry audio: 16 kHz mono PCM16 in MVP, Opus later.
- **Text frames** carry JSON events, each with a `type`:
  - from the client: `session.start`, `session.end`, `client.mute`
  - from the server: `transcript.partial`, `transcript.final`, `agent.audio.start`, `agent.audio.end`, `agent.interrupted`, `plan.node`, `decision`, `latency`, `error`, `session.ended`
- Event schemas live in `contracts/media/*.json` and are checked by contract tests on both sides.
