-- Add execution mode fields to waves.
ALTER TABLE waves ADD COLUMN mode TEXT NOT NULL DEFAULT 'loop';
ALTER TABLE waves ADD COLUMN loop_flow TEXT NOT NULL DEFAULT 'ship-roadmap';
ALTER TABLE waves ADD COLUMN cron TEXT;

-- Make stimulus_id nullable in pending_activations and activation_log.
-- SQLite doesn't support ALTER COLUMN, so we recreate the tables.

-- pending_activations: drop FK constraint on stimulus_id, make nullable.
CREATE TABLE pending_activations_new (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    stimulus_id TEXT,
    source INTEGER NOT NULL DEFAULT 0,
    reason TEXT NOT NULL DEFAULT '',
    from_sha TEXT NOT NULL DEFAULT '',
    to_sha TEXT NOT NULL DEFAULT '',
    queued_at BIGINT NOT NULL,
    target_branch TEXT NOT NULL DEFAULT 'main'
);
INSERT INTO pending_activations_new
    SELECT id, wave_id, stimulus_id, source, reason, from_sha, to_sha, queued_at, target_branch
    FROM pending_activations;
DROP TABLE pending_activations;
ALTER TABLE pending_activations_new RENAME TO pending_activations;
CREATE INDEX IF NOT EXISTS idx_pending_wave_id ON pending_activations(wave_id);

-- activation_log: make stimulus_id nullable.
CREATE TABLE activation_log_new (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    stimulus_id TEXT,
    source INTEGER NOT NULL DEFAULT 0,
    reason TEXT NOT NULL DEFAULT '',
    outcome TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
INSERT INTO activation_log_new
    SELECT id, wave_id, stimulus_id, source, reason, outcome, created_at
    FROM activation_log;
DROP TABLE activation_log;
ALTER TABLE activation_log_new RENAME TO activation_log;
CREATE INDEX IF NOT EXISTS idx_activation_log_wave_id ON activation_log(wave_id);
