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

-- Work and time belong to the input on its origin. Transcripts and usage
-- share this selection even when a Task's Project is only known here.
CREATE VIEW machine_record_inputs AS
    SELECT r.rowid AS record_id, r.origin_machine_id, r.kind, r.key, r.row,
        json_extract(r.row,'$.session_id') AS session_id,
        json_extract(r.row,'$.task_id') AS task_id,
        json_extract(r.row,'$.wave_id') AS wave_id,
        coalesce(json_extract(r.row,'$.wave_id'),p.wave_id) AS scope_wave,
        p.id AS project_id,p.project_slug,p.external_project_id,
        coalesce(json_extract(r.row,'$.started_at'),json_extract(r.row,'$.observed_at'),
            json_extract(r.row,'$.usage.observed_at'),json_extract(r.row,'$.ended_at')) AS started
    FROM machine_records r
    LEFT JOIN tasks t ON t.id=json_extract(r.row,'$.task_id')
    LEFT JOIN projects p ON p.id=coalesce(json_extract(r.row,'$.project_id'),t.project_id)
    WHERE CASE r.kind WHEN 'turn' THEN 1
        WHEN 'session_event' THEN json_extract(r.row,'$.kind')='captured'
        WHEN 'history' THEN json_extract(r.row,'$.kind')='manifest' END;

CREATE INDEX machine_record_turn_end ON machine_records(json_extract(row,'$.ended_at'))
    WHERE kind='turn' AND json_extract(row,'$.ended_at') IS NOT NULL;

CREATE TRIGGER store_revision_machine_records_insert AFTER INSERT ON machine_records
WHEN NEW.kind='turn' BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='sessions';
END;
CREATE TRIGGER store_revision_machine_records_update AFTER UPDATE ON machine_records
WHEN OLD.kind='turn' OR NEW.kind='turn' BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='sessions';
END;
CREATE TRIGGER store_revision_machine_records_delete AFTER DELETE ON machine_records
WHEN OLD.kind='turn' BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='sessions';
END;

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
