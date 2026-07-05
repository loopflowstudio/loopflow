ALTER TABLE terminal_sessions ADD COLUMN session_use TEXT NOT NULL DEFAULT 'palette';

UPDATE terminal_sessions
SET session_use = 'worker'
WHERE run_id IS NOT NULL
  AND step LIKE 'dispatch:%';

UPDATE terminal_sessions
SET session_use = 'wave_agent'
WHERE source = 'wave_agent'
   OR step LIKE 'goal:%';
