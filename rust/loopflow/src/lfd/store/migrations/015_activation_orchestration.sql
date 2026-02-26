ALTER TABLE pending_activations ADD COLUMN source INTEGER NOT NULL DEFAULT 0;
ALTER TABLE pending_activations ADD COLUMN reason TEXT NOT NULL DEFAULT '';

CREATE TABLE activation_log (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    stimulus_id TEXT NOT NULL REFERENCES stimuli(id) ON DELETE CASCADE,
    source INTEGER NOT NULL,
    reason TEXT NOT NULL DEFAULT '',
    outcome TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE INDEX idx_activation_log_wave ON activation_log(wave_id, created_at DESC);

ALTER TABLE wave_runs ADD COLUMN activation_log_id TEXT REFERENCES activation_log(id);
