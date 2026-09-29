-- name: flow_operation_history
-- id: 8a533c4383974c1599d1ccf076665c63
-- depends_on: select_flow_history, own_conversation_ancestry

CREATE TABLE flow_events_next (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    flow_id TEXT NOT NULL REFERENCES flow_sessions(id),
    version INTEGER,
    node INTEGER NOT NULL,
    iterations TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('selected','consumed','operation_started','operation_completed')),
    session_event INTEGER REFERENCES session_events(seq),
    observed_at INTEGER,
    exec_id TEXT REFERENCES execs(id),
    operation_start INTEGER REFERENCES flow_events_next(seq),
    outcome TEXT,
    payload TEXT,
    UNIQUE(flow_id,kind,session_event),
    UNIQUE(operation_start),
    CHECK((kind IN ('selected','consumed')) = (session_event IS NOT NULL)),
    CHECK((kind='operation_completed') = (operation_start IS NOT NULL AND outcome IS NOT NULL))
);
INSERT INTO flow_events_next(seq,flow_id,version,node,iterations,kind,session_event,observed_at,exec_id)
SELECT seq,flow_id,version,node,iterations,kind,session_event,observed_at,
    (SELECT exec_id FROM session_events WHERE seq=flow_events.session_event)
FROM flow_events;
DROP TABLE flow_events;
ALTER TABLE flow_events_next RENAME TO flow_events;
CREATE INDEX flow_events_history ON flow_events(flow_id,seq);
ALTER TABLE flow_sessions ADD COLUMN operation_start INTEGER REFERENCES flow_events(seq);

-- A reservation timestamp is not an observed operation start. Keep its original
-- meaning and selector in the imported payload; do not manufacture an Exec.
INSERT INTO flow_events(flow_id,version,node,iterations,kind,payload)
SELECT r.invocation_id,CASE WHEN f.current_run_id=r.id THEN f.position_version END,
    r.node,r.iterations,'operation_started',json_object('legacy_run_id',r.id,'reserved_at',r.created_at)
FROM runs r JOIN flow_sessions f ON f.id=r.invocation_id
WHERE r.provider='loopflow' AND r.skill IS NULL AND r.published=1
    AND r.node IS NOT NULL AND r.iterations IS NOT NULL;
INSERT INTO flow_events(flow_id,version,node,iterations,kind,operation_start,outcome,observed_at,payload)
SELECT e.flow_id,e.version,e.node,e.iterations,'operation_completed',e.seq,r.outcome,r.ended_at,e.payload
FROM flow_events e JOIN runs r ON r.id=json_extract(e.payload,'$.legacy_run_id')
WHERE e.kind='operation_started' AND r.outcome IS NOT NULL;
UPDATE flow_sessions SET operation_start=(SELECT seq FROM flow_events
    WHERE kind='operation_started' AND json_extract(payload,'$.legacy_run_id')=flow_sessions.current_run_id);
UPDATE flow_sessions SET current_run_id=NULL WHERE operation_start IS NOT NULL;

DROP TRIGGER validate_task_started_update;
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
