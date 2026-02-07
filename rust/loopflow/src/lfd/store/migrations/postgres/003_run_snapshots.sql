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

UPDATE wave_runs
SET snapshot_repo = waves.repo,
    snapshot_flow = waves.flow,
    snapshot_direction = waves.direction,
    snapshot_area = waves.area
FROM waves
WHERE wave_runs.wave_id = waves.id
  AND wave_runs.snapshot_repo = '';

UPDATE meta SET value = '4' WHERE key = 'schema_version';
