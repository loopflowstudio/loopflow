-- draft: session_moves
-- A Session moved to another Machine is completed here and names the Machine
-- it went to, so resuming it here follows it there.
ALTER TABLE agent_sessions ADD COLUMN moved_to_machine_id TEXT;
