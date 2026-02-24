DROP INDEX idx_waves_parent;
DROP INDEX idx_waves_top_level_name;
DROP INDEX idx_waves_child_name;

ALTER TABLE waves DROP COLUMN wave_type;
ALTER TABLE waves DROP COLUMN parent_wave_id;
ALTER TABLE waves DROP COLUMN position;

CREATE TABLE chords (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL
);

CREATE UNIQUE INDEX idx_chords_default
ON chords(is_default)
WHERE is_default = 1;

CREATE TABLE chord_members (
    chord_id TEXT NOT NULL REFERENCES chords(id) ON DELETE CASCADE,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    PRIMARY KEY (chord_id, wave_id)
);

CREATE INDEX idx_chord_members_wave_id
ON chord_members(wave_id);

-- Recreate stimuli with nullable cron and new source_wave_id column.
-- SQLite cannot ALTER COLUMN, so we recreate the table.
-- pending_activations references stimuli(id), so recreate it too.

CREATE TABLE pending_activations_bak AS SELECT * FROM pending_activations;
DROP TABLE pending_activations;

CREATE TABLE stimuli_new (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    kind INTEGER NOT NULL,
    cron TEXT,
    last_main_sha TEXT,
    last_triggered_at BIGINT,
    created_at BIGINT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    source_wave_id TEXT REFERENCES waves(id) ON DELETE SET NULL
);

INSERT INTO stimuli_new (id, wave_id, kind, cron, last_main_sha, last_triggered_at, created_at, enabled)
SELECT id, wave_id, kind, NULLIF(cron, ''), last_main_sha, last_triggered_at, created_at, enabled
FROM stimuli;

DROP TABLE stimuli;
ALTER TABLE stimuli_new RENAME TO stimuli;

CREATE INDEX idx_stimuli_wave_id ON stimuli(wave_id);
CREATE INDEX idx_stimuli_kind ON stimuli(kind);
CREATE INDEX idx_stimuli_source_wave_id ON stimuli(source_wave_id);

CREATE TABLE pending_activations (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    stimulus_id TEXT NOT NULL REFERENCES stimuli(id) ON DELETE CASCADE,
    from_sha TEXT NOT NULL DEFAULT '',
    to_sha TEXT NOT NULL DEFAULT '',
    queued_at BIGINT NOT NULL
);

INSERT INTO pending_activations SELECT * FROM pending_activations_bak;
DROP TABLE pending_activations_bak;

CREATE INDEX idx_pending_wave_id ON pending_activations(wave_id);
