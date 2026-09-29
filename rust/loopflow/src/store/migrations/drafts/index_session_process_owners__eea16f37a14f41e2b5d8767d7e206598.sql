-- name: index_session_process_owners
-- id: eea16f37a14f41e2b5d8767d7e206598
-- depends_on: agent_session_admission

-- Observe only Sessions related to sampled Execs, provider PIDs or native clients.
CREATE INDEX session_driver_exec ON agent_sessions(driver_exec_id) WHERE driver_exec_id IS NOT NULL;
CREATE INDEX session_provider_pid ON agent_sessions(provider_pid) WHERE provider_pid IS NOT NULL;
CREATE INDEX session_unknown_engine_origin ON agent_sessions(provider_exec_id) WHERE provider_pid IS NULL;
