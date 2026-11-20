-- A Task conversation that died comes back on its own; one the person quit
-- stays quit. The server's recovery pass counts its starts since the
-- conversation last finished a turn (`starts_since_turn`), when it last
-- started one (`last_started_at`), and why the last start failed
-- (`start_failure`, NULL once a start succeeds).
ALTER TABLE agent_sessions ADD COLUMN starts_since_turn INTEGER NOT NULL DEFAULT 0;
ALTER TABLE agent_sessions ADD COLUMN last_started_at INTEGER;
ALTER TABLE agent_sessions ADD COLUMN start_failure TEXT;
-- A Task run whose carrier (`lf task run`) died is carried again. The pass
-- counts those starts since the run last finished a step (`restarts`) and
-- when it last started one (`restarted_at`).
ALTER TABLE task_graphs ADD COLUMN restarts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE task_graphs ADD COLUMN restarted_at INTEGER;
-- A carrier that exits clears its OS process. One found dead has it cleared
-- for it, and `run_died_at` keeps that it died until the next run starts.
ALTER TABLE task_graphs ADD COLUMN run_died_at INTEGER;
