-- A Task conversation that died comes back on its own; one the person quit
-- stays quit. The server's recovery pass counts its starts since the
-- conversation last finished a turn (`recovery_attempts`), when it last
-- started one (`recovered_at`), and why the last start failed
-- (`recovery_failure`, NULL once a start succeeds).
ALTER TABLE agent_sessions ADD COLUMN recovery_attempts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE agent_sessions ADD COLUMN recovered_at INTEGER;
ALTER TABLE agent_sessions ADD COLUMN recovery_failure TEXT;
-- A Task run whose carrier (`lf task run`) died is carried again. The pass
-- counts those starts since the run last finished a step (`restarts`) and
-- when it last started one (`restarted_at`).
ALTER TABLE task_graphs ADD COLUMN restarts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE task_graphs ADD COLUMN restarted_at INTEGER;
