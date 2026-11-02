-- The saved plan of a draft (M3): NULL until the operator saves one. The domain checks the
-- plan on every read and write; the CHECK repeats its outline, so no client can store
-- something that isn't a plan object.
ALTER TABLE agent_drafts
    ADD CONSTRAINT agent_drafts_plan_shape CHECK (
        plan IS NULL
        OR (
            jsonb_typeof(plan) = 'object'
            AND jsonb_typeof(plan -> 'nodes') = 'array'
            AND jsonb_typeof(plan -> 'edges') = 'array'
        )
    );
