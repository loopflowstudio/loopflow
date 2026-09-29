-- name: record_native_input
-- id: e2670f219b194c2698e8a9c310b6bd31
-- depends_on: retain_input_sql_evidence

-- Only an observed start records its input. Recovery never borrows a later input.
-- Existing starts have no exact recorded input relation and remain unknown.
ALTER TABLE session_events ADD COLUMN input_id TEXT REFERENCES agent_session_inputs(input_id);
CREATE INDEX session_event_input ON session_events(session_id,input_id) WHERE kind='started';
