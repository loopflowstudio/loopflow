-- name: record_session_output
-- id: 98630201d91945deac760c629972912d
-- depends_on: index_session_metadata, runtime_flow_children

-- Preserve native output independently of completion and retain all historical references.
DROP TRIGGER validate_task_started_update;
CREATE TABLE session_output_history (
    seq INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES agent_sessions(id),
    provider_thread TEXT,
    provider_turn TEXT,
    kind TEXT NOT NULL CHECK(kind IN ('started','usage','completed','observed','output')),
    receipt_key TEXT NOT NULL,
    provider_generation INTEGER,
    exec_id TEXT REFERENCES execs(id),
    task_id TEXT REFERENCES tasks(id),
    wave_id TEXT REFERENCES waves(id),
    observed_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    input_id TEXT REFERENCES agent_session_inputs(input_id)
);
INSERT INTO session_output_history SELECT * FROM session_events;
DROP TABLE session_events;
ALTER TABLE session_output_history RENAME TO session_events;
CREATE UNIQUE INDEX session_event_receipt ON session_events(
    session_id, COALESCE(provider_thread,''), COALESCE(provider_turn,''), kind, receipt_key);
CREATE INDEX session_events_history ON session_events(session_id,seq);
CREATE INDEX session_events_turn ON session_events(session_id,provider_thread,provider_turn,seq);
CREATE TRIGGER task_first_agent_turn AFTER INSERT ON session_events
WHEN NEW.kind='started' AND NEW.task_id IS NOT NULL BEGIN
    UPDATE tasks SET started_at=NEW.observed_at WHERE id=NEW.task_id AND started_at IS NULL;
END;
CREATE INDEX session_event_input ON session_events(session_id,input_id) WHERE kind='started';
CREATE TRIGGER retain_task_agent_turn BEFORE DELETE ON session_events
WHEN OLD.kind='started' AND OLD.task_id IS NOT NULL
    AND NOT EXISTS(SELECT 1 FROM agent_session_inputs WHERE historical_task_id=OLD.task_id)
    AND NOT EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=OLD.task_id AND seq!=OLD.seq)
BEGIN SELECT RAISE(ABORT,'Cannot remove the last work of a started Task'); END;


CREATE TRIGGER validate_task_started_update BEFORE UPDATE OF started_at ON tasks BEGIN
    SELECT CASE WHEN OLD.started_at IS NOT NULL AND NEW.started_at IS NOT OLD.started_at
        THEN RAISE(ABORT,'Task first assignment time cannot change') END;
    SELECT CASE WHEN (NEW.started_at IS NOT NULL) != (
        EXISTS(SELECT 1 FROM agent_session_inputs WHERE historical_task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM flow_sessions f JOIN flow_events e ON e.flow_id=f.id
            WHERE f.task_id=NEW.id AND e.kind='operation_started') OR
        EXISTS(SELECT 1 FROM task_events e WHERE e.task_id=NEW.id
            AND json_extract(e.kind_json,'$.kind')='started')
    ) THEN RAISE(ABORT,'Started requires recorded Task work') END;
END;

CREATE INDEX session_input_membership ON session_events(
    session_id, receipt_key, CASE WHEN json_valid(payload) THEN CASE WHEN json_extract(payload,'$.source')='manifest.json' AND json_extract(payload,'$.evidence.schema_version')=1 AND json_extract(payload,'$.evidence.run_id')=json_extract(payload,'$.input_id') AND receipt_key=json_extract(payload,'$.input_id')||':manifest.json' THEN json_extract(payload,'$.evidence.flow.kind') END END)
    WHERE kind='observed' AND substr(receipt_key,-14)=':manifest.json';
