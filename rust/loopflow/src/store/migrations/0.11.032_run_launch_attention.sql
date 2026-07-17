-- Make product Run authority and opaque surfaces first-class on the existing
-- launch/turn lineage. Nullable control columns keep unattributed historical
-- trace captures honest instead of fabricating Run ownership for them.

ALTER TABLE agent_launches ADD COLUMN product_run_id TEXT REFERENCES runs(id) ON DELETE RESTRICT;
ALTER TABLE agent_launches ADD COLUMN home_id TEXT REFERENCES homes(id) ON DELETE RESTRICT;
ALTER TABLE agent_launches ADD COLUMN account_id TEXT;
ALTER TABLE agent_launches ADD COLUMN launch_state TEXT CHECK (
    launch_state IN ('starting', 'live', 'stopping', 'ended')
);
ALTER TABLE agent_launches ADD COLUMN containment_kind TEXT CHECK (
    containment_kind IN ('process_group', 'tmux')
);
ALTER TABLE agent_launches ADD COLUMN containment_id TEXT;
ALTER TABLE agent_launches ADD COLUMN resume_token TEXT;
ALTER TABLE agent_launches ADD COLUMN opaque_epoch_id TEXT REFERENCES epochs(id) ON DELETE RESTRICT;
ALTER TABLE agent_launches ADD COLUMN opaque_basis_rev INTEGER;
ALTER TABLE agent_launches ADD COLUMN attention_kind TEXT CHECK (
    attention_kind IN ('user', 'parent')
);
ALTER TABLE agent_launches ADD COLUMN attention_work_kind TEXT CHECK (
    attention_work_kind IN ('wave', 'project', 'task')
);
ALTER TABLE agent_launches ADD COLUMN attention_work_id TEXT;
ALTER TABLE agent_launches ADD COLUMN attention_at INTEGER;
ALTER TABLE agent_launches ADD COLUMN handback_state TEXT CHECK (
    handback_state IN ('succeeded', 'failed', 'interrupted', 'unknown')
);

UPDATE agent_launches SET resume_token = provider_session_id
WHERE provider_session_id IS NOT NULL;

CREATE UNIQUE INDEX idx_agent_launches_one_control_live
    ON agent_launches(product_run_id)
    WHERE launch_state IN ('starting', 'live', 'stopping');
CREATE INDEX idx_agent_launches_attention
    ON agent_launches(attention_kind, attention_work_kind, attention_work_id, attention_at)
    WHERE attention_kind IS NOT NULL;

CREATE UNIQUE INDEX idx_homes_route ON homes(route);

CREATE TABLE work_flow_positions (
    epoch_id TEXT PRIMARY KEY REFERENCES epochs(id) ON DELETE CASCADE,
    flow TEXT NOT NULL CHECK (length(trim(flow)) > 0),
    step TEXT NOT NULL CHECK (length(trim(step)) > 0),
    step_index INTEGER NOT NULL CHECK (step_index >= 0),
    iteration INTEGER NOT NULL CHECK (iteration >= 0),
    interactive INTEGER NOT NULL CHECK (interactive IN (0, 1)),
    updated_at INTEGER NOT NULL
);

CREATE TABLE done_proposals (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
    epoch_id TEXT NOT NULL REFERENCES epochs(id) ON DELETE RESTRICT,
    basis_rev INTEGER NOT NULL,
    proposed_at INTEGER NOT NULL,
    UNIQUE (run_id, epoch_id, basis_rev),
    FOREIGN KEY (epoch_id, basis_rev)
        REFERENCES epoch_revisions(epoch_id, rev) ON DELETE RESTRICT
);
