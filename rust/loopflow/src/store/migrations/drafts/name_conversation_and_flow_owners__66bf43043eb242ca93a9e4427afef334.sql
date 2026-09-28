-- name: name_conversation_and_flow_owners
-- id: 66bf43043eb242ca93a9e4427afef334
-- depends_on: drop_wave_services, project_status_chapters, record_session_events

-- Rename the existing owners in place. SQLite carries foreign keys, indexes and
-- trigger references across; captures, claims and conversation history stay put.
ALTER TABLE sessions RENAME TO agent_sessions;
ALTER TABLE flow_invocations RENAME TO flow_sessions;
