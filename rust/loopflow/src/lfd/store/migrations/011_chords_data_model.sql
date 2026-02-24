DROP TABLE IF EXISTS wave_queue_blocks;
DROP TABLE IF EXISTS wave_pr_merge_events;
DROP TABLE IF EXISTS chat_messages;
DROP TABLE IF EXISTS chat_memory_blocks;
DROP TABLE IF EXISTS summaries;
DROP TABLE IF EXISTS fork_runs;
DROP TABLE IF EXISTS pending_activations;
DROP TABLE IF EXISTS wave_runs;
DROP TABLE IF EXISTS stimuli;
DROP TABLE IF EXISTS agents;
DROP TABLE IF EXISTS live_pr_states;
DROP TABLE IF EXISTS waves;

CREATE TABLE waves (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    repo TEXT NOT NULL,
    flow TEXT NOT NULL,
    direction TEXT NOT NULL DEFAULT '[]',
    area TEXT NOT NULL DEFAULT '[]',
    paused INTEGER NOT NULL,
    status INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    created_at BIGINT NOT NULL,
    schema_ref TEXT,
    schema_name TEXT,
    wave_type INTEGER NOT NULL DEFAULT 1,
    parent_wave_id TEXT REFERENCES waves(id) ON DELETE CASCADE,
    position INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_waves_name ON waves(name);
CREATE INDEX idx_waves_repo ON waves(repo);
CREATE INDEX idx_waves_parent ON waves(parent_wave_id);
CREATE UNIQUE INDEX idx_waves_top_level_name
ON waves(repo, name)
WHERE parent_wave_id IS NULL;
CREATE UNIQUE INDEX idx_waves_child_name
ON waves(parent_wave_id, name)
WHERE parent_wave_id IS NOT NULL;

CREATE TABLE agents (
    id TEXT PRIMARY KEY,
    step TEXT NOT NULL,
    repo TEXT NOT NULL,
    worktree TEXT NOT NULL,
    wave_run_id TEXT,
    status INTEGER NOT NULL,
    started_at BIGINT NOT NULL,
    ended_at BIGINT,
    pid INTEGER,
    model TEXT NOT NULL,
    run_mode TEXT NOT NULL,
    container_id TEXT
);

CREATE INDEX idx_agents_status ON agents(status);

CREATE TABLE stimuli (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    kind INTEGER NOT NULL,
    cron TEXT NOT NULL DEFAULT '',
    last_main_sha TEXT,
    last_triggered_at BIGINT,
    created_at BIGINT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX idx_stimuli_wave_id ON stimuli(wave_id);
CREATE INDEX idx_stimuli_kind ON stimuli(kind);

CREATE TABLE pending_activations (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    stimulus_id TEXT NOT NULL REFERENCES stimuli(id) ON DELETE CASCADE,
    from_sha TEXT NOT NULL DEFAULT '',
    to_sha TEXT NOT NULL DEFAULT '',
    queued_at BIGINT NOT NULL
);

CREATE INDEX idx_pending_wave_id ON pending_activations(wave_id);

CREATE TABLE wave_runs (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    iteration INTEGER NOT NULL,
    step_index INTEGER NOT NULL DEFAULT 0,
    status INTEGER NOT NULL,
    worktree TEXT NOT NULL DEFAULT '',
    branch TEXT NOT NULL DEFAULT '',
    started_at BIGINT NOT NULL,
    ended_at BIGINT,
    error TEXT,
    snapshot_repo TEXT NOT NULL DEFAULT '',
    snapshot_flow TEXT NOT NULL DEFAULT '',
    snapshot_direction TEXT NOT NULL DEFAULT '[]',
    snapshot_area TEXT NOT NULL DEFAULT '[]',
    snapshot_pr TEXT,
    flow_parents TEXT NOT NULL DEFAULT '[]',
    run_kind INTEGER NOT NULL DEFAULT 1,
    sidecar_kind INTEGER,
    parent_run_id TEXT,
    parent_pr_number BIGINT,
    stack_position INTEGER NOT NULL DEFAULT 0,
    stack_group_id TEXT NOT NULL DEFAULT '',
    stack_status INTEGER NOT NULL DEFAULT 1,
    lineage_inferred INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_wave_runs_wave_id ON wave_runs(wave_id, started_at);
CREATE INDEX idx_wave_runs_wave_id_kind_status
ON wave_runs(wave_id, run_kind, status, started_at);
CREATE INDEX idx_wave_runs_wave_id_stack_position
ON wave_runs(wave_id, stack_position);
CREATE INDEX idx_wave_runs_parent_run_id
ON wave_runs(parent_run_id);

CREATE TABLE fork_runs (
    id TEXT PRIMARY KEY,
    wave_run_id TEXT REFERENCES wave_runs(id) ON DELETE CASCADE,
    step_index INTEGER NOT NULL,
    branch_index INTEGER NOT NULL,
    status INTEGER NOT NULL,
    worktree TEXT NOT NULL
);

CREATE INDEX idx_fork_runs_wave_run_id ON fork_runs(wave_run_id, step_index);

CREATE TABLE summaries (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    source_hash TEXT NOT NULL,
    token_budget INTEGER NOT NULL,
    model TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE UNIQUE INDEX idx_summaries_wave_id ON summaries(wave_id);

CREATE TABLE live_pr_states (
    repo_id TEXT NOT NULL,
    pr_number BIGINT NOT NULL,
    state INTEGER NOT NULL,
    is_draft INTEGER NOT NULL DEFAULT 0,
    head_ref TEXT NOT NULL DEFAULT '',
    head_sha TEXT NOT NULL DEFAULT '',
    base_ref TEXT NOT NULL DEFAULT '',
    updated_at BIGINT NOT NULL,
    merged_at BIGINT,
    synced_at BIGINT NOT NULL,
    PRIMARY KEY (repo_id, pr_number)
);

CREATE INDEX idx_live_pr_states_state ON live_pr_states(state);

CREATE TABLE wave_queue_blocks (
    wave_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    attempted_at BIGINT NOT NULL,
    conflict_files TEXT NOT NULL DEFAULT '[]',
    error TEXT,
    PRIMARY KEY (wave_id, run_id),
    FOREIGN KEY (wave_id) REFERENCES waves(id) ON DELETE CASCADE,
    FOREIGN KEY (run_id) REFERENCES wave_runs(id) ON DELETE CASCADE
);

CREATE INDEX idx_wave_queue_blocks_wave_id
ON wave_queue_blocks(wave_id);

CREATE TABLE wave_pr_merge_events (
    wave_id TEXT NOT NULL,
    pr_number BIGINT NOT NULL,
    merged_at BIGINT NOT NULL,
    processed_at BIGINT NOT NULL,
    PRIMARY KEY (wave_id, pr_number, merged_at),
    FOREIGN KEY (wave_id) REFERENCES waves(id) ON DELETE CASCADE
);

CREATE INDEX idx_wave_pr_merge_events_wave_id
ON wave_pr_merge_events(wave_id, processed_at);

CREATE TABLE chat_memory_blocks (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    position INTEGER NOT NULL,
    updated_at BIGINT NOT NULL,
    PRIMARY KEY (wave_id, name)
);

CREATE INDEX idx_chat_memory_blocks_wave_pos
ON chat_memory_blocks(wave_id, position);

CREATE TABLE chat_messages (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE INDEX idx_chat_messages_wave_id
ON chat_messages(wave_id, created_at);
