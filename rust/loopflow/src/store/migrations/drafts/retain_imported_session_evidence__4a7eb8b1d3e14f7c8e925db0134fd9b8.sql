-- name: retain_imported_session_evidence
-- id: 4a7eb8b1d3e14f7c8e925db0134fd9b8
-- depends_on: agent_session_admission

-- Legacy observations are history, never invented native turns or Execs.
DROP TRIGGER validate_task_started_update;
CREATE TABLE imported_session_evidence (
    seq INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES agent_sessions(id),
    provider_thread TEXT,
    provider_turn TEXT,
    kind TEXT NOT NULL CHECK(kind IN ('started','usage','completed','observed')),
    receipt_key TEXT NOT NULL,
    provider_generation INTEGER,
    exec_id TEXT REFERENCES execs(id),
    task_id TEXT REFERENCES tasks(id),
    wave_id TEXT REFERENCES waves(id),
    observed_at INTEGER NOT NULL,
    payload TEXT NOT NULL
);
INSERT INTO imported_session_evidence SELECT * FROM session_events;
DROP TABLE session_events;
ALTER TABLE imported_session_evidence RENAME TO session_events;
CREATE UNIQUE INDEX session_event_receipt ON session_events(
    session_id, COALESCE(provider_thread,''), COALESCE(provider_turn,''), kind, receipt_key);
CREATE INDEX session_events_history ON session_events(session_id,seq);
CREATE INDEX session_events_turn ON session_events(session_id,provider_thread,provider_turn,seq);
CREATE TRIGGER task_first_agent_turn AFTER INSERT ON session_events
WHEN NEW.kind='started' AND NEW.task_id IS NOT NULL BEGIN
    UPDATE tasks SET started_at=NEW.observed_at WHERE id=NEW.task_id AND started_at IS NULL;
END;
CREATE TRIGGER retain_task_agent_turn BEFORE DELETE ON session_events
WHEN OLD.kind='started' AND OLD.task_id IS NOT NULL
    AND NOT EXISTS(SELECT 1 FROM runs WHERE task_id=OLD.task_id)
    AND NOT EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=OLD.task_id AND seq!=OLD.seq)
BEGIN SELECT RAISE(ABORT,'Cannot remove the last work of a started Task'); END;

CREATE TRIGGER validate_task_started_update BEFORE UPDATE OF started_at ON tasks BEGIN
    SELECT CASE WHEN OLD.started_at IS NOT NULL AND NEW.started_at IS NOT OLD.started_at
        THEN RAISE(ABORT,'Task first assignment time cannot change') END;
    SELECT CASE WHEN (NEW.started_at IS NOT NULL) != (
        EXISTS(SELECT 1 FROM runs WHERE task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM flow_sessions f JOIN flow_events e ON e.flow_id=f.id
            WHERE f.task_id=NEW.id AND e.kind='operation_started')
    ) THEN RAISE(ABORT,'Started requires recorded Task work') END;
END;
