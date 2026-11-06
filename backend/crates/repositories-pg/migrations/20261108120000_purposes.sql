-- Purposes (M3b): the workspace's own purposes, their drafts and their immutable versions,
-- and the purpose version every agent pins. The six built-in system purposes live in code
-- (vak_domain::Purpose::system, fixed IDs 019a0000-0000-7000-8000-00000000000N), so they can't
-- drift from the templates and rule packs; only agents refer to them here.

CREATE TABLE purposes (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    -- The draft's name, for lists; the domain checks it against the draft on every read.
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
    draft jsonb NOT NULL CHECK (jsonb_typeof(draft) = 'object'),
    draft_hash text NOT NULL CHECK (draft_hash ~ '^sha256:[0-9a-f]{64}$'),
    -- 0 until the first version is saved.
    latest_version integer NOT NULL DEFAULT 0 CHECK (latest_version >= 0),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_at timestamptz NOT NULL,
    created_by uuid NOT NULL REFERENCES operators (id),
    updated_at timestamptz NOT NULL,
    -- The system purposes' IDs are reserved.
    CONSTRAINT purposes_not_system CHECK (
        id NOT BETWEEN '019a0000-0000-7000-8000-000000000000'
        AND '019a0000-0000-7000-8000-0000000000ff'
    )
);

-- The purpose list: a workspace's purposes, most recently updated first.
CREATE INDEX purposes_by_recency ON purposes (workspace_id, updated_at DESC, id DESC);

-- A saved version: the definition as it was, never changed or removed, so an agent that pins
-- it always finds it.
CREATE TABLE purpose_versions (
    purpose_id uuid NOT NULL REFERENCES purposes (id),
    version integer NOT NULL CHECK (version > 0),
    definition jsonb NOT NULL CHECK (jsonb_typeof(definition) = 'object'),
    content_hash text NOT NULL CHECK (content_hash ~ '^sha256:[0-9a-f]{64}$'),
    created_at timestamptz NOT NULL,
    created_by uuid NOT NULL REFERENCES operators (id),
    PRIMARY KEY (purpose_id, version)
);

CREATE FUNCTION purpose_versions_append_only() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'purpose_versions is append-only: % is not allowed', TG_OP
        USING ERRCODE = 'insufficient_privilege';
END;
$$;

CREATE TRIGGER purpose_versions_no_update_or_delete
    BEFORE UPDATE OR DELETE ON purpose_versions
    FOR EACH ROW EXECUTE FUNCTION purpose_versions_append_only();

CREATE TRIGGER purpose_versions_no_truncate
    BEFORE TRUNCATE ON purpose_versions
    FOR EACH STATEMENT EXECUTE FUNCTION purpose_versions_append_only();

-- Every agent pins a purpose version. Agents made before purposes pin their use case's
-- system purpose, version 1: the same template, rule pack and instructions they had, and
-- (the domain encodes that pin as nothing) the same content hash.
ALTER TABLE agent_drafts
    ADD COLUMN purpose_id uuid,
    ADD COLUMN purpose_version integer;

UPDATE agent_drafts AS d
SET purpose_id = CASE a.use_case
        WHEN 'support' THEN '019a0000-0000-7000-8000-000000000001'::uuid
        WHEN 'service' THEN '019a0000-0000-7000-8000-000000000002'::uuid
        WHEN 'booking' THEN '019a0000-0000-7000-8000-000000000003'::uuid
        WHEN 'survey' THEN '019a0000-0000-7000-8000-000000000004'::uuid
        WHEN 'telemarketing' THEN '019a0000-0000-7000-8000-000000000005'::uuid
        WHEN 'custom' THEN '019a0000-0000-7000-8000-000000000006'::uuid
    END,
    purpose_version = 1
FROM agents AS a
WHERE a.id = d.agent_id;

ALTER TABLE agent_drafts
    ALTER COLUMN purpose_id SET NOT NULL,
    ALTER COLUMN purpose_version SET NOT NULL,
    ADD CONSTRAINT agent_drafts_purpose_version CHECK (purpose_version > 0);

-- "Used by N agents": active agents per purpose.
CREATE INDEX agent_drafts_by_purpose ON agent_drafts (purpose_id);
