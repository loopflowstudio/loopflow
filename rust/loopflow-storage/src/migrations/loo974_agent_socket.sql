-- Where an idle OpenCode or Pi work session takes steers: the Unix socket its
-- plugin or extension listens on. `agent_inbox` keeps Claude's inbox key file.
ALTER TABLE agent_sessions ADD COLUMN agent_socket TEXT;
