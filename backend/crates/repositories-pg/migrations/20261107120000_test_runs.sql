-- M5 Test: scenarios saved with a draft, and test runs.

-- The draft's saved test scenarios: NULL until the operator saves some (the use case's
-- defaults apply until then). The domain checks every scenario on read and write; the CHECK
-- repeats the outline.
ALTER TABLE agent_drafts
    ADD COLUMN scenarios jsonb,
    ADD CONSTRAINT agent_drafts_scenarios_shape CHECK (
        scenarios IS NULL OR jsonb_typeof(scenarios) = 'array'
    );

-- A test run: one job that plays an agent's scenarios against simulated callers. Its results
-- (each scenario's definition, status, transcript and rubric checks) are one JSONB array,
-- written as a whole on every step, which the domain checks on every read. Transcripts are
-- personal data: they live here and nowhere else (never in logs).
CREATE TABLE test_runs (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    agent_id uuid NOT NULL REFERENCES agents (id),
    created_by uuid NOT NULL REFERENCES operators (id),
    -- The draft it tests: the hash when queued, then the one it started on.
    draft_hash text NOT NULL CHECK (draft_hash ~ '^sha256:[0-9a-f]{64}$'),
    status text NOT NULL CHECK (
        status IN ('queued', 'running', 'finished', 'cancelled', 'interrupted')
    ),
    -- Only a finished run has a verdict.
    verdict text CHECK (verdict IN ('passed', 'failed')),
    -- The models it used and its sampling seed, set when it starts.
    llm_model text CHECK (char_length(llm_model) BETWEEN 1 AND 256),
    decision_model text CHECK (char_length(decision_model) BETWEEN 1 AND 256),
    seed bigint,
    scenarios jsonb NOT NULL CHECK (jsonb_typeof(scenarios) = 'array'),
    -- Optimistic concurrency between the worker and a cancel.
    revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_at timestamptz NOT NULL,
    started_at timestamptz,
    finished_at timestamptz,
    CONSTRAINT test_runs_verdict_when_finished CHECK ((status = 'finished') = (verdict IS NOT NULL)),
    CONSTRAINT test_runs_models_together CHECK (
        (llm_model IS NULL) = (decision_model IS NULL) AND (llm_model IS NULL) = (seed IS NULL)
    )
);

-- An agent has at most one queued or running run.
CREATE UNIQUE INDEX test_runs_one_active_per_agent
    ON test_runs (agent_id)
    WHERE status IN ('queued', 'running');

-- The agent's runs, newest first.
CREATE INDEX test_runs_by_agent ON test_runs (agent_id, created_at DESC, id DESC);

-- The latest finished run on a draft (setup progress).
CREATE INDEX test_runs_finished_by_draft
    ON test_runs (agent_id, draft_hash, created_at DESC, id DESC)
    WHERE status = 'finished';

-- The queue, oldest first.
CREATE INDEX test_runs_queued ON test_runs (created_at, id) WHERE status = 'queued';
