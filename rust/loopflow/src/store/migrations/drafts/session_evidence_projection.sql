-- Raw references are a rebuildable projection of retained history, never a
-- cached filesystem destination or a checkout-disposal decision.
CREATE INDEX session_evidence_sources ON session_events(seq)
WHERE kind='captured' OR (kind='observed' AND
    (receipt_key GLOB '*:runs' OR receipt_key GLOB '*:manifest.json' OR receipt_key GLOB '*:terminal.json'));

CREATE VIEW session_evidence_source AS
SELECT seq,
    CASE WHEN kind='captured' THEN receipt_key
         WHEN receipt_key GLOB '*:runs' THEN substr(receipt_key,1,length(receipt_key)-5)
         ELSE substr(receipt_key,1,length(receipt_key)-14) END AS capture_key,
    CASE WHEN kind='captured' THEN '[]'
         WHEN json_valid(payload) THEN CASE WHEN json_type(payload,'$.evidence')='object' THEN json_extract(payload,
            '$.evidence.artifact_dir', '$.evidence.conversation_path',
            '$.evidence.provider_events_path', '$.evidence.provider_session_path',
            '$.evidence.runtime_path', '$.evidence.result_ref', '$.evidence.context.path') ELSE 'null' END
         ELSE 'null' END AS raw_paths
FROM session_events
WHERE kind='captured' OR (kind='observed' AND
    (receipt_key GLOB '*:runs' OR receipt_key GLOB '*:manifest.json' OR receipt_key GLOB '*:terminal.json'));

-- Every projected or re-projected reference gets a later, never reused revision.
-- A long observation revalidates what changed behind it by reading past the
-- last revision it consumed, instead of restarting or trusting a frozen snapshot.
CREATE TABLE session_evidence (
    revision INTEGER PRIMARY KEY AUTOINCREMENT,
    event_seq INTEGER NOT NULL UNIQUE REFERENCES session_events(seq) ON DELETE CASCADE,
    capture_key TEXT,
    raw_paths TEXT NOT NULL
);
CREATE TABLE session_evidence_backfill (
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    through_seq INTEGER NOT NULL,
    target_seq INTEGER NOT NULL,
    complete INTEGER NOT NULL CHECK(complete IN (0,1))
);
INSERT INTO session_evidence_backfill
VALUES(1,0,COALESCE((SELECT MAX(seq) FROM session_events),0),NOT EXISTS(SELECT 1 FROM session_evidence_source));

-- Appends, edits and removals maintain the projection in the source transaction.
-- No parsing failure can reject durable evidence: malformed paths remain raw and
-- make the cleanup reader fail closed, including during a partial backfill.
CREATE TRIGGER session_evidence_insert AFTER INSERT ON session_events
BEGIN
    INSERT INTO session_evidence(event_seq,capture_key,raw_paths)
    SELECT seq,capture_key,raw_paths FROM session_evidence_source WHERE seq=NEW.seq;
END;
CREATE TRIGGER session_evidence_update AFTER UPDATE OF seq,kind,receipt_key,payload ON session_events
BEGIN
    DELETE FROM session_evidence WHERE event_seq=OLD.seq;
    INSERT INTO session_evidence(event_seq,capture_key,raw_paths)
    SELECT seq,capture_key,raw_paths FROM session_evidence_source WHERE seq=NEW.seq;
END;
CREATE TRIGGER session_evidence_delete AFTER DELETE ON session_events
BEGIN
    DELETE FROM session_evidence WHERE event_seq=OLD.seq;
END;
