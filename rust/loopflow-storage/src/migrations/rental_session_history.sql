-- Extend the kinds that a rental can leave behind. Keep older copies verbatim.
CREATE TABLE machine_records_expanded (
    origin_machine_id TEXT NOT NULL REFERENCES machines(id),
    kind TEXT NOT NULL CHECK(kind IN ('process','session','session_event','graph_process',
        'graph_process_step','history','turn','transcript','checkpoint')),
    key TEXT NOT NULL,
    row TEXT NOT NULL CHECK(json_valid(row)),
    observed_at INTEGER NOT NULL,
    PRIMARY KEY(origin_machine_id,kind,key)
);
INSERT INTO machine_records_expanded(origin_machine_id,kind,key,row,observed_at)
    SELECT origin_machine_id,kind,key,row,observed_at FROM machine_records;
DROP TABLE machine_records;
ALTER TABLE machine_records_expanded RENAME TO machine_records;
CREATE INDEX machine_record_session ON machine_records(origin_machine_id,kind,json_extract(row,'$.session_id'));
CREATE INDEX machine_record_lookup ON machine_records(kind,key,observed_at);

-- Search by the input time on its origin. Collection can happen days later.
CREATE INDEX machine_record_input_time ON machine_records(coalesce(json_extract(row,'$.started_at'),json_extract(row,'$.observed_at'),
        json_extract(row,'$.usage.observed_at'),json_extract(row,'$.ended_at')))
    WHERE CASE kind WHEN 'turn' THEN 1
        WHEN 'session_event' THEN json_extract(row,'$.kind')='captured'
        WHEN 'history' THEN json_extract(row,'$.kind')='manifest' END;
CREATE INDEX machine_record_input_task ON machine_records(json_extract(row,'$.task_id'),
    coalesce(json_extract(row,'$.started_at'),json_extract(row,'$.observed_at'),
        json_extract(row,'$.usage.observed_at'),json_extract(row,'$.ended_at')))
    WHERE CASE kind WHEN 'turn' THEN 1
        WHEN 'session_event' THEN json_extract(row,'$.kind')='captured'
        WHEN 'history' THEN json_extract(row,'$.kind')='manifest' END;

-- One position per Session, regardless of how many times its state changed.
CREATE TABLE session_export_changes (
    position INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL UNIQUE
);
CREATE TRIGGER session_export_insert AFTER INSERT ON agent_sessions BEGIN
    INSERT INTO session_export_changes(session_id) VALUES(NEW.id)
        ON CONFLICT(session_id) DO UPDATE SET position=(SELECT MAX(position)+1 FROM session_export_changes);
END;
CREATE TRIGGER session_export_update AFTER UPDATE ON agent_sessions BEGIN
    INSERT INTO session_export_changes(session_id) VALUES(NEW.id)
        ON CONFLICT(session_id) DO UPDATE SET position=(SELECT MAX(position)+1 FROM session_export_changes);
END;
CREATE TRIGGER session_event_export_insert AFTER INSERT ON session_events BEGIN
    INSERT INTO session_export_changes(session_id) VALUES(NEW.session_id)
        ON CONFLICT(session_id) DO UPDATE SET position=(SELECT MAX(position)+1 FROM session_export_changes);
END;
CREATE TRIGGER session_event_export_update AFTER UPDATE ON session_events BEGIN
    INSERT INTO session_export_changes(session_id) VALUES(NEW.session_id)
        ON CONFLICT(session_id) DO UPDATE SET position=(SELECT MAX(position)+1 FROM session_export_changes);
END;
