CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT INTO meta (key, value)
VALUES ('schema_version', '1')
ON CONFLICT (key) DO NOTHING;

CREATE TABLE IF NOT EXISTS waves (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    repo TEXT NOT NULL,
    flow TEXT NOT NULL,
    direction JSONB NOT NULL DEFAULT '[]'::jsonb,
    area JSONB NOT NULL DEFAULT '[]'::jsonb,
    stimulus_kind INTEGER NOT NULL DEFAULT 1,
    stimulus_cron TEXT NOT NULL DEFAULT '',
    paused BOOLEAN NOT NULL,
    status INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    worktree TEXT NOT NULL DEFAULT '',
    branch TEXT NOT NULL DEFAULT '',
    pr_limit INTEGER NOT NULL,
    merge_mode INTEGER NOT NULL,
    pid INTEGER,
    created_at BIGINT NOT NULL,
    last_main_sha TEXT,
    consecutive_failures INTEGER NOT NULL,
    pending_activations INTEGER NOT NULL,
    step_index INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_waves_repo ON waves(repo);
CREATE INDEX IF NOT EXISTS idx_waves_status ON waves(status);

CREATE TABLE IF NOT EXISTS step_runs (
    id TEXT PRIMARY KEY,
    step TEXT NOT NULL,
    repo TEXT NOT NULL,
    worktree TEXT NOT NULL,
    flow_run_id TEXT,
    wave_id TEXT,
    status INTEGER NOT NULL,
    started_at BIGINT NOT NULL,
    ended_at BIGINT,
    pid INTEGER,
    model TEXT NOT NULL,
    run_mode TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS stimuli (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    kind INTEGER NOT NULL,
    cron TEXT NOT NULL DEFAULT '',
    last_main_sha TEXT,
    last_triggered_at BIGINT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
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

CREATE TABLE IF NOT EXISTS fork_runs (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    step_index INTEGER NOT NULL,
    branch_index INTEGER NOT NULL,
    status INTEGER NOT NULL,
    worktree TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_fork_runs_wave_id ON fork_runs(wave_id, step_index);

CREATE INDEX IF NOT EXISTS idx_step_runs_status ON step_runs(status);
CREATE INDEX IF NOT EXISTS idx_step_runs_wave ON step_runs(wave_id);
