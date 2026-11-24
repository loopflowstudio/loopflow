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
