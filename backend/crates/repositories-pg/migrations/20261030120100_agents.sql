-- Agents and their drafts. The columns repeat the domain's rules as CHECK constraints, so a
-- row the domain would reject can't be written by any client.

CREATE TABLE agents (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
    use_case text NOT NULL CHECK (
        use_case IN ('support', 'service', 'booking', 'survey', 'telemarketing', 'custom')
    ),
    jurisdiction text NOT NULL CHECK (jurisdiction ~ '^[A-Z]{2}(-[A-Z0-9]{1,3})?$'),
    -- Optimistic concurrency: an update names the revision it read.
    revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_at timestamptz NOT NULL,
    created_by uuid NOT NULL REFERENCES operators (id),
    updated_at timestamptz NOT NULL,
    archived_at timestamptz
);

-- The agent list: active agents of a workspace, most recently updated first.
CREATE INDEX agents_active_by_recency
    ON agents (workspace_id, updated_at DESC, id DESC)
    WHERE archived_at IS NULL;

-- One draft per agent. Persona and settings are JSONB that the domain types validate on every
-- read and write; content_hash is the domain's SHA-256 over the whole draft.
CREATE TABLE agent_drafts (
    agent_id uuid PRIMARY KEY REFERENCES agents (id),
    persona jsonb NOT NULL CHECK (jsonb_typeof(persona) = 'object'),
    plan jsonb,
    settings jsonb NOT NULL CHECK (jsonb_typeof(settings) = 'object'),
    rule_pack_id text NOT NULL CHECK (rule_pack_id ~ '^[a-z0-9-]{1,64}$'),
    rule_pack_version integer NOT NULL CHECK (rule_pack_version > 0),
    content_hash text NOT NULL CHECK (content_hash ~ '^sha256:[0-9a-f]{64}$'),
    updated_at timestamptz NOT NULL,
    updated_by uuid NOT NULL REFERENCES operators (id)
);
