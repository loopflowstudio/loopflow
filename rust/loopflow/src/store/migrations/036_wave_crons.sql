CREATE TABLE IF NOT EXISTS wave_crons (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    flow TEXT NOT NULL,
    schedule TEXT NOT NULL,
    last_triggered_at BIGINT,
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_wave_crons_wave ON wave_crons(wave_id);

INSERT INTO wave_crons (id, wave_id, flow, schedule, last_triggered_at, created_at)
SELECT id || ':primary-cron', id, primary_flow, cron, NULL, created_at
FROM waves
WHERE mode = 'cron' AND cron IS NOT NULL AND cron != '';

UPDATE waves
SET mode = 'manual', workers = 0
WHERE mode = 'cron';

ALTER TABLE waves DROP COLUMN cron;
