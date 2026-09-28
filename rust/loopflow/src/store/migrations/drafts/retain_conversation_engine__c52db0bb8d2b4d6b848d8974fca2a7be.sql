-- name: retain_conversation_engine
-- id: c52db0bb8d2b4d6b848d8974fca2a7be
-- depends_on: name_conversation_and_flow_owners

-- Operational evidence for the native engine, independent of its lf driver.
-- Missing evidence is unknown; a socket address does not establish liveness.
ALTER TABLE agent_sessions ADD COLUMN provider_pid INTEGER;
ALTER TABLE agent_sessions ADD COLUMN provider_started_at INTEGER;
