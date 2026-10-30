-- Audit events: who changed what, and when. Append-only: triggers reject UPDATE, DELETE and
-- TRUNCATE for every role, so a record can't be rewritten after the fact.

CREATE TABLE audit_events (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    at timestamptz NOT NULL,
    actor_id uuid NOT NULL REFERENCES operators (id),
    -- The operator's email when the event happened.
    actor text NOT NULL,
    action text NOT NULL CHECK (action ~ '^[a-z_]+\.[a-z_]+$'),
    entity text NOT NULL CHECK (entity ~ '^[a-z_]+$'),
    entity_id uuid NOT NULL,
    details jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(details) = 'object')
);

CREATE INDEX audit_events_by_entity ON audit_events (entity, entity_id, at, id);

CREATE FUNCTION audit_events_append_only() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'audit_events is append-only: % is not allowed', TG_OP
        USING ERRCODE = 'insufficient_privilege';
END;
$$;

CREATE TRIGGER audit_events_no_update_or_delete
    BEFORE UPDATE OR DELETE ON audit_events
    FOR EACH ROW EXECUTE FUNCTION audit_events_append_only();

CREATE TRIGGER audit_events_no_truncate
    BEFORE TRUNCATE ON audit_events
    FOR EACH STATEMENT EXECUTE FUNCTION audit_events_append_only();
