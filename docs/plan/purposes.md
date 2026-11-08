# Purposes (M3b)

Status: **design**, 2026-11-06. Milestone [M3b](milestones.md#m3b-purposes). Built on the owner's request:

> I want to be able to have a place to build these purposes and orchestrate how these would work exactly like which tools, guardrails allowed and not allowed etc.

The owner decided three things:

1. **Tools, first version:** the platform's built-in tools, plus the owner's own HTTP tools.
2. **Versions:** each save of a purpose makes a new immutable version. Agents pin a version and show "Update available" until the operator updates, so a tested or published agent never changes behind their back.
3. **Order:** purposes come before M4b. M4b then carries out the tools and enforces the guardrails in calls.

M3b **defines, edits, versions and links** purposes. It executes no tool and adds no crate, adapter or `reqwest` use. Calls get one thing from it now: the purpose's guardrails and tool descriptions are compiled into the LLM instructions of live test calls, so the owner can try them today.

## 1. What a purpose is

A purpose is a workspace-wide, versioned definition of what an agent is for and how it may work:

| Part | Holds | Rules |
|---|---|---|
| Overview | Name (1–80 characters), description (up to 500), default persona prompt (up to 2,000) | The persona replaces the use-case sentence in the agent's instructions |
| Compliance class | One of today's six use cases (`support`, `service`, `booking`, `survey`, `telemarketing`, `custom`) | Combined with the agent's jurisdiction, it selects the rule pack: `RulePackRef::for_agent(class, jurisdiction)`, exactly as `use_case` does today |
| Starting flow | A plan graph, edited with the existing flow editor | Stored **without** the locked texts: an agent's start step opens with its own pack's texts, which differ by jurisdiction. A version needs a flow that passes every graph rule |
| Tools | Built-in tools to turn on, and up to 10 custom HTTP tools | §3 |
| Guardrails | Allowed topics, never-rules, escalation rules, limits | §4 |
| Compliance additions | Extra opt-out phrases, extra forbidden claims, shorter recording and transcript retention | Tighten-only, §5 |

### Draft and versions

A purpose has one **draft**, autosaved like an agent's draft, and numbered **versions**. "Save version N" freezes the draft as version N:

- A version is immutable. Postgres rejects `UPDATE`, `DELETE` and `TRUNCATE` on `purpose_versions` with a trigger, like `audit_events`.
- Saving needs a change since the latest version (`PurposeError::Unchanged`) and a draft without problems: every flow graph rule holds and every HTTP tool's host is allowed (`PurposeError::Invalid`). A half-edited draft can be stored; it just can't become a version.
- Version numbers start at 1 and go up by one. A new purpose has a draft and no version yet; agents can only pin a saved version.

### System purposes

The six use cases become **built-in system purposes** at version 1. They live in code (`vak_domain::Purpose::system`), like the rule packs and templates do today, so they can't drift from them. Each has a fixed UUID v7 (`019a0000-0000-7000-8000-00000000000N`, N = 1–6 in `UseCase::ALL` order):

- name: today's picker labels (Customer support, Customer service, Appointment booking, Survey, Outbound sales, Something else)
- compliance class: its use case
- persona: today's use-case sentence ("Your job: answer customers' questions and help solve their problems.")
- flow: `Plan::template(use_case, &[])`, so `flow.opened_with(pack.locked_texts()) == Plan::template(use_case, pack.locked_texts())` for every pack
- tools: only `mark_do_not_call`; guardrails: none (30-minute calls); compliance additions: none, so the effective policy is exactly the pack

They are read-only and can be **duplicated** to customize. A duplicate starts as a draft copy without a version.

### Data

| Table | Key columns | Notes |
|---|---|---|
| `purposes` | id, workspace_id, name, draft JSONB, draft_hash, latest_version (0 = none), revision, created_at/by, updated_at | Custom purposes only. Optimistic concurrency like `agents` |
| `purpose_versions` | (purpose_id, version), definition JSONB, content_hash, created_at, created_by | Append-only (trigger). JSONB read back through the domain's constructors; the hash is recomputed and compared |
| `agent_drafts` | + purpose_id, purpose_version | NOT NULL. The migration sets every existing agent to its use case's system purpose, version 1 |

Migration `20261108120000_purposes.sql`. No foreign key from `agent_drafts.purpose_id`: system purposes are code, not rows; versions are never deleted, so a pinned custom version always resolves.

## 2. Agents and purposes

An agent's draft pins `(purpose id, version)` (`AgentDraft::purpose`). The compliance class stays in `use_case`, so the rule pack, the existing API field `useCase` and the agent list keep working.

- **Create.** `POST /agents` with only `useCase` (the pinned `agent-provisioning` body) takes that use case's system purpose v1. An optional `purposeId` takes that purpose's latest version; `useCase` must then be its class. The agent copies nothing at creation, and nothing is saved: until the operator saves a plan, the agent's plan **is** its pinned version's starting flow, opened with its pack's locked texts (`DraftPlan::for_purpose`). So a new agent stays at 25 %, as `agent-provisioning` and `plan` v1 require.
- **Persona.** The agent's instructions use the pinned version's persona. A version never changes, so this behaves like a copy, with one source for the update account below. A per-agent persona override can come later.
- **Switching purpose** (the Purpose picker, `PATCH /agents/{id}` with `purposeId` or `useCase`) takes the other purpose's latest version and keeps a saved plan, as changing the use case does today.
- **Content hash.** The draft hash adds `purpose.id` and `purpose.version` lines only when the agent isn't on its use case's system purpose v1. Every existing agent is, so every stored hash stays valid and no row needs rehashing.

### "Update available"

When the pinned purpose has a newer version, `GET /agents/{id}/purpose` answers an `update` with the new version number and an account of what changes, area by area (name, description, compliance class, persona, starting flow, tools, guardrails, compliance additions). The agent's header shows a secondary "Update available" button; its dialog lists the changes and says what happens to the plan:

| The agent's plan | On update |
|---|---|
| Not saved | Follows the purpose: the new version's flow is shown |
| Saved, and still exactly the old version's flow | Replaced with the new version's flow, so it keeps following the purpose |
| Saved with the operator's edits | **Kept**, and the dialog says so. The operator can choose "Replace my plan with the new starting flow" instead. An edited plan is never overwritten silently |

`POST /agents/{id}/purpose/update` with `{version, replacePlan}` moves the agent. It writes one `agent.draft_updated` audit event (fields `purpose`, and `use_case`, `rule_pack` and `plan` when they change). A class change moves the rule pack too; a kept plan is then checked against the new pack, as on a use-case change today.

### Seam for M5

Test scenarios (M5) will attach to purposes: a version is the natural unit to test against. M3b leaves the seam only: versions are immutable and addressable as `(id, version)`. It adds no scenario field.

## 3. Tools

### Built-in catalog

Only tools the platform can carry out itself, with no outside integration:

| Name | What it will do (M4b) |
|---|---|
| `transfer_to_human` | Hand the call to a person. In browser test calls it ends the call with a handoff message and flags it; a real transfer comes with telephony (L1) |
| `end_call` | End the call politely |
| `record_answer` | Store a survey answer with the call |
| `schedule_callback` | Note a time to call back, stored with the call |
| `mark_do_not_call` | The opt-out tool. **Always on and locked**: every purpose has it, and no edit can remove it |

### Custom HTTP tools

| Field | Rule |
|---|---|
| `name` | An LLM-safe identifier: 1–64 lowercase letters, digits or `_`, starting with a letter. Unique in the purpose, and not a built-in name |
| `description` | What it does, 1–500 characters: the LLM reads it |
| `inputs` | A small JSON Schema subset: `{type: "object", properties: {<name>: {type, description}}, required: [...]}`; types `string`, `number`, `integer`, `boolean`; at most 10 properties |
| `method` | `GET`, `POST`, `PUT`, `PATCH` or `DELETE` |
| `url` | `https://` only, no user info or fragment, a DNS host name (no IP literal, no single-label host such as `localhost`), at most 2,048 characters. The host must be on the tool host allow-list |
| `timeoutMs` | 100 to 10,000, default 3,000. A voice reply can't wait longer |
| `secret` | Optional. The **name** of a secret in the workspace's secret store: a key in the app's Databricks secret scope in production, an entry in `.env` locally. Never a value: the API has no field for one, and the UI never shows one |
| `confirm` | Whether the agent must confirm with the caller before it calls the tool |

Booking against a real calendar is a custom HTTP tool.

**The allow-list** is deployment configuration, `VAK_TOOL_HOSTS` (comma-separated host names, `*.example.com` for subdomains), not an operator setting: it bounds where agents may send call data, so widening it is a reviewed change. `GET /catalog/tools` shows it read-only. A tool whose host is not on it can be stored in the draft, but blocks "Save version". Removing a host later doesn't change saved versions; M4b refuses to call it.

### What M4b adds

M3b stores and validates definitions only. M4b executes tools and needs the owner to add a tools adapter crate to `backend/layers.toml` and `backend/deny.toml` (the `reqwest` wrapper). It brings:

- SSRF checks at call time: resolve the host, refuse private, loopback, link-local and metadata addresses, pin the resolved address for the request (DNS rebinding), and refuse redirects
- response caps (size, content type) and the timeout above
- secret resolution from the secret store, sent as `Authorization: Bearer <secret>`, never logged; only from a scope (or name prefix) reserved for tool secrets, so a tool can never name a platform secret such as the database URL
- the host allow-list checked again at call time, not only when a version is saved
- the LLM tool specs from `inputs`, and the confirmation turn for `confirm` tools

## 4. Guardrails

| Guardrail | Holds | M3b (live test calls) | M4b |
|---|---|---|---|
| Allowed topics | Up to 20 topics, 1–200 characters each. Empty means no limit | Compiled into the instructions: stay on these topics, steer back off-topic requests | Decision-engine check |
| Never | Up to 20 things the agent must never say or do ("Promise refunds", "Give medical advice"). The pack's forbidden claims show beside them, locked | Compiled into the instructions | Decision-engine check |
| Escalation rules | Up to 5. *When*: a yes/no question for the decision engine ("Does the caller ask for a human?"). *Then*: `transfer`, `end_call` with a message said word for word, or `flag` for review | Stored and validated only | Asked and acted on |
| Limits | Maximum call length, 1–30 minutes (default 30, the platform cap). Personal data the agent must never ask for: card numbers, national ID numbers (SSN), passwords and PINs, bank account numbers, health details | The call length caps the session (`SessionConfig::with_max_duration`); the never-ask list is compiled into the instructions | Decision-engine check |

Services compile the instructions in `SessionScript::for_agent(draft, purpose)`; the pipeline actor doesn't change. A system purpose compiles to exactly today's instructions. Extra opt-out phrases join the phrase list (layer 1 of the opt-out) at once.

**Latency budget for M4b.** One decision request with four questions takes about 500 ms locally, one question 130–260 ms, and the reply waits at most 300 ms for the opt-out answer. So escalation questions must never block a reply: M4b asks them in their own decision request beside the reply (or after it), and acts on a yes at the next turn boundary. Only the opt-out question stays on the reply's critical path. Five escalation rules is the cap for that reason.

## 5. Compliance is tighten-only

The pack's locked texts (AI disclosure, recording notice, purpose and opt-out notices), its opt-out phrases and confirmation, and its forbidden claims always apply. A purpose can only add:

| Addition | Effective policy (`EffectiveCompliance::of(pack, additions)`) |
|---|---|
| Opt-out phrases (up to 20) | The pack's phrases, then the extra ones (lower case, without duplicates) |
| Forbidden claims (up to 20) | The pack's claims, then the extra ones |
| Recording retention cap (days) | The shorter of the pack's period and the cap |
| Transcript retention cap (days) | The shorter of the pack's period and the cap |
| Audit retention | Always the pack's: a purpose has no field for it, so it can never be shorter |

The locked texts and the opt-out confirmation are the pack's, unchanged. Because the combination is a min/union, no addition can weaken the pack, whatever its value; a property test pins that. The Compliance section shows the pack's baseline read-only (per shipped jurisdiction of the class) next to the additions and the effective result.

## 6. API (additive)

All under `/api/v1`, with the forwarded identity, problem+json errors and the OpenAPI document. Purpose routes accept bodies up to 512 KiB (flows).

| Route | `operationId` | Does |
|---|---|---|
| `GET /purposes` | `listPurposes` | System purposes first (in use-case order), then custom ones, newest first; each with `latestVersion`, `unsavedChanges` and `usedBy` (active agents) |
| `POST /purposes` | `createPurpose` | Duplicate: `{duplicateOf, version?, name?}` (default: the latest version, "Copy of …"). 201 with `Location` |
| `GET /purposes/{id}` | `getPurpose` | The draft (without the flow), its problems, and the compliance baselines with the effective policy |
| `PATCH /purposes/{id}` | `updatePurpose` | Autosave: any of `name`, `description`, `complianceClass`, `persona`, `tools`, `guardrails`, `compliance` (each section replaced whole). 409 on a system purpose |
| `GET`, `PUT /purposes/{id}/flow` | `getPurposeFlow`, `savePurposeFlow` | The starting flow, in the agent plan's shape (`AgentPlan`, `PlanInput`); validation covers the graph rules |
| `POST /purposes/{id}/versions` | `savePurposeVersion` | Freeze the draft as the next version. 409 when read-only or unchanged, 422 with the problems |
| `GET /catalog/tools` | `listTools` | The built-in tools and the tool host allow-list |
| `GET /agents/{id}/purpose` | `getAgentPurpose` | The pinned purpose and version, and the `update` (version, changes, plan outcome) when a newer version exists |
| `POST /agents/{id}/purpose/update` | `updateAgentPurpose` | Move to `{version, replacePlan}` |

`POST /agents` and `PATCH /agents/{id}` gain an optional `purposeId`; `Agent` and `AgentSummary` gain `purpose: {id, version, system}`. The create body still requires exactly `name`, `useCase` and `voice`, so the breaking-change check passes.

## 7. UI

- **Navigation:** "Purposes" between Agents and Settings.
- **Purposes list** (`/purposes`): built-in and custom purposes as rows with their compliance class, version and "Used by N agents". One primary action, "New purpose": a dialog to name it and pick the purpose to start from.
- **Purpose page** (`/purposes/$purposeId?section=…`): the agent page's cockpit style. From `lg` the document never scrolls; the section scrolls on its own. A header with the name, the version state ("Version 2", "Changes since version 2", "Not saved as a version yet"), the autosave status and the one primary action: "Save version N" (custom) or "Duplicate" (built-in), plus "Create agent" once a version exists. Sections: **Overview**, **Flow** (the plans feature's flow editor, composed in by the route; read-only for built-in purposes), **Tools**, **Guardrails** and **Compliance**.
- **The agent's Purpose picker** lists the workspace's purposes. Up to 7 it is a native select (HICK-3; the `agent-provisioning` v1 contract sees the six built-ins and selects `support`); past 7 it becomes a searchable combobox. Built-in options keep their use case as the value, so the pinned contracts pass unchanged.
- **Agent header:** "Update available" (secondary) when a newer version exists, opening the update dialog of §2.

## 8. Out of scope (later)

- Executing tools, SSRF checks at call time, response caps and secret resolution (M4b, with the tools adapter).
- Enforcing guardrails and escalations with the decision engine (M4b).
- Test scenarios attached to purposes (M5).
- Archiving custom purposes, a per-agent persona override, and an operator-editable allow-list (with multi-tenancy).
