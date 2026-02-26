ALTER TABLE stimuli ADD COLUMN signal INTEGER;
UPDATE stimuli SET signal = kind;
DROP INDEX IF EXISTS idx_stimuli_kind;
ALTER TABLE stimuli DROP COLUMN kind;
ALTER TABLE stimuli ADD COLUMN flow TEXT;
CREATE INDEX IF NOT EXISTS idx_stimuli_signal ON stimuli(signal);

DROP INDEX IF EXISTS idx_wave_runs_wave_id_kind_status;
ALTER TABLE wave_runs DROP COLUMN run_kind;
ALTER TABLE wave_runs DROP COLUMN ci_fix_kind;
CREATE INDEX IF NOT EXISTS idx_wave_runs_wave_id_status
ON wave_runs(wave_id, status, started_at);
