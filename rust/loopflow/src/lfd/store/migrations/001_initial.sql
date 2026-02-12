CREATE TABLE IF NOT EXISTS waves (
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
    UNIQUE(name, repo)
);

CREATE INDEX IF NOT EXISTS idx_waves_name ON waves(name);
CREATE INDEX IF NOT EXISTS idx_waves_repo ON waves(repo);

CREATE TABLE IF NOT EXISTS agents (
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
    run_mode TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status);

CREATE TABLE IF NOT EXISTS stimuli (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    kind INTEGER NOT NULL,
    cron TEXT NOT NULL DEFAULT '',
    last_main_sha TEXT,
    last_triggered_at BIGINT,
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_stimuli_wave_id ON stimuli(wave_id);
CREATE INDEX IF NOT EXISTS idx_stimuli_kind ON stimuli(kind);

CREATE TABLE IF NOT EXISTS pending_activations (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    stimulus_id TEXT NOT NULL REFERENCES stimuli(id) ON DELETE CASCADE,
    from_sha TEXT NOT NULL DEFAULT '',
    to_sha TEXT NOT NULL DEFAULT '',
    queued_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pending_wave_id ON pending_activations(wave_id);

CREATE TABLE IF NOT EXISTS wave_runs (
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
    flow_parents TEXT NOT NULL DEFAULT '[]'
);

CREATE INDEX IF NOT EXISTS idx_wave_runs_wave_id ON wave_runs(wave_id, started_at);

CREATE TABLE IF NOT EXISTS fork_runs (
    id TEXT PRIMARY KEY,
    wave_run_id TEXT REFERENCES wave_runs(id) ON DELETE CASCADE,
    step_index INTEGER NOT NULL,
    branch_index INTEGER NOT NULL,
    status INTEGER NOT NULL,
    worktree TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_fork_runs_wave_run_id ON fork_runs(wave_run_id, step_index);

CREATE TABLE IF NOT EXISTS summaries (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    source_hash TEXT NOT NULL,
    token_budget INTEGER NOT NULL,
    model TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_summaries_wave_id ON summaries(wave_id);
