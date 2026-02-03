ALTER TABLE wave_runs
    ADD COLUMN IF NOT EXISTS flow_parents JSONB NOT NULL DEFAULT '[]'::jsonb;

UPDATE meta SET value = '3' WHERE key = 'schema_version';
