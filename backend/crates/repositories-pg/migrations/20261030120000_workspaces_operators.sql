-- Workspaces and the operators who sign in to them. The MVP has one workspace; the column
-- exists on every table so multi-tenancy is a data change, not a schema change.

CREATE TABLE workspaces (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
    created_at timestamptz NOT NULL DEFAULT now()
);

-- The default workspace. vak-repositories-pg refers to it by this fixed ID.
INSERT INTO workspaces (id, name)
VALUES ('01923a00-0000-7000-8000-000000000001', 'Default');

-- Operators come from the identity Databricks Apps forwards (X-Forwarded-User and
-- X-Forwarded-Email). A row is created or refreshed by the first change an operator makes.
CREATE TABLE operators (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    external_id text NOT NULL CHECK (char_length(external_id) BETWEEN 1 AND 256),
    email text NOT NULL CHECK (char_length(email) BETWEEN 3 AND 254),
    created_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, external_id)
);
