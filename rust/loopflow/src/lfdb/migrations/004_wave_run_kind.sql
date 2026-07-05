ALTER TABLE runs
ADD COLUMN run_kind INTEGER NOT NULL DEFAULT 1;

ALTER TABLE runs
ADD COLUMN sidecar_kind INTEGER;

CREATE INDEX IF NOT EXISTS idx_runs_wave_id_kind_status
ON runs(wave_id, run_kind, status, started_at);
