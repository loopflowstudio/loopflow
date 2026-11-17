-- A Task conversation that died comes back on its own; one the person quit
-- stays quit. The server's recovery pass counts its starts since the
-- conversation last finished a turn (`recovery_attempts`), when it last
-- started one (`recovered_at`), and why the last start failed
-- (`recovery_failure`, NULL once a start succeeds).
ALTER TABLE agent_sessions ADD COLUMN recovery_attempts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE agent_sessions ADD COLUMN recovered_at INTEGER;
ALTER TABLE agent_sessions ADD COLUMN recovery_failure TEXT;
