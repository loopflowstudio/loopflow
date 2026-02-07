ALTER TABLE wave_runs
    ADD COLUMN IF NOT EXISTS snapshot_repo TEXT NOT NULL DEFAULT '';

ALTER TABLE wave_runs
    ADD COLUMN IF NOT EXISTS snapshot_flow TEXT NOT NULL DEFAULT '';

ALTER TABLE wave_runs
    ADD COLUMN IF NOT EXISTS snapshot_direction JSONB NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE wave_runs
    ADD COLUMN IF NOT EXISTS snapshot_area JSONB NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE wave_runs
    ADD COLUMN IF NOT EXISTS snapshot_pr JSONB;

CREATE INDEX IF NOT EXISTS idx_waves_name ON waves(name);

UPDATE meta SET value = '4' WHERE key = 'schema_version';
