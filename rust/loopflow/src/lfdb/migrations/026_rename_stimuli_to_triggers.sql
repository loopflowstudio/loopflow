-- Rename stimuli table to triggers.
ALTER TABLE stimuli RENAME TO triggers;

-- Remap signal integers: Watch(3)→Repo(1), Listen(5)→Wave(2), CiFailure(6)→CiFailure(3).
-- Delete any Unspecified(0) rows — they were never functional.
DELETE FROM triggers WHERE signal = 0;
UPDATE triggers SET signal = CASE signal
    WHEN 3 THEN 1
    WHEN 5 THEN 2
    WHEN 6 THEN 3
END WHERE signal IN (3, 5, 6);

-- Drop cron column (cron is now a wave-level property, not a trigger property).
ALTER TABLE triggers DROP COLUMN cron;

-- Rename stimulus_id → trigger_id in pending_activations; drop source column.
CREATE TABLE pending_activations_new (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    trigger_id TEXT,
    reason TEXT NOT NULL DEFAULT '',
    from_sha TEXT NOT NULL DEFAULT '',
    to_sha TEXT NOT NULL DEFAULT '',
    queued_at BIGINT NOT NULL,
    target_branch TEXT NOT NULL DEFAULT 'main'
);
INSERT INTO pending_activations_new (id, wave_id, trigger_id, reason, from_sha, to_sha, queued_at, target_branch)
    SELECT id, wave_id, stimulus_id, reason, from_sha, to_sha, queued_at, target_branch
    FROM pending_activations;
DROP TABLE pending_activations;
ALTER TABLE pending_activations_new RENAME TO pending_activations;
CREATE INDEX IF NOT EXISTS idx_pending_wave_id ON pending_activations(wave_id);

-- Rename stimulus_id → trigger_id in activation_log; drop source column.
CREATE TABLE activation_log_new (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    trigger_id TEXT,
    reason TEXT NOT NULL DEFAULT '',
    outcome TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
INSERT INTO activation_log_new (id, wave_id, trigger_id, reason, outcome, created_at)
    SELECT id, wave_id, stimulus_id, reason, outcome, created_at
    FROM activation_log;
DROP TABLE activation_log;
ALTER TABLE activation_log_new RENAME TO activation_log;
CREATE INDEX IF NOT EXISTS idx_activation_log_wave_id ON activation_log(wave_id);
