-- Why the holder intentionally closed this generation of a Session.
ALTER TABLE agent_sessions ADD COLUMN agent_end_reason TEXT;
