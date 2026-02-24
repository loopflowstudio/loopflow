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

ALTER TABLE stimuli
ADD COLUMN source_wave_id TEXT REFERENCES waves(id) ON DELETE SET NULL;

CREATE INDEX idx_stimuli_source_wave_id
ON stimuli(source_wave_id);
