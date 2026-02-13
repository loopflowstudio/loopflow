ALTER TABLE wave_runs
ADD COLUMN run_kind INTEGER NOT NULL DEFAULT 1;

ALTER TABLE wave_runs
ADD COLUMN sidecar_kind INTEGER;

CREATE INDEX IF NOT EXISTS idx_wave_runs_wave_id_kind_status
ON wave_runs(wave_id, run_kind, status, started_at);
