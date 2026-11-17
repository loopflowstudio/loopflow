-- A Task conversation that died comes back on its own; one the person quit
-- stays quit. `agent_stopped_at` is when lf ended the current agent at a
-- person's request, and the next agent clears it. The server's recovery pass
-- counts its starts since the conversation last finished a turn
-- (`recovery_attempts`) and when it last started one (`recovered_at`).
ALTER TABLE agent_sessions ADD COLUMN agent_stopped_at INTEGER;
ALTER TABLE agent_sessions ADD COLUMN recovery_attempts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE agent_sessions ADD COLUMN recovered_at INTEGER;
