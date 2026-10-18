-- depends_on: session_transport
-- A Trace owns one journal directory. Case aliases may have distinct directories
-- even when no Process primary keys collide. Never choose one and lose the other.
CREATE TEMP TABLE canonical_identity_conflicts (domain TEXT, first TEXT, second TEXT);
CREATE TEMP TRIGGER canonical_identity_conflict BEFORE INSERT ON canonical_identity_conflicts
BEGIN
    SELECT RAISE(ABORT, 'canonical identity collision: ' || NEW.domain || ': '
        || NEW.first || ' / ' || NEW.second);
END;
INSERT INTO canonical_identity_conflicts
SELECT 'Trace journal locator',min(trace_id),max(trace_id)
FROM processes GROUP BY lower(trace_id) HAVING count(DISTINCT trace_id)>1;
DROP TRIGGER canonical_identity_conflict;
DROP TABLE canonical_identity_conflicts;

-- Filesystem addresses are opaque locators on their existing owners. Retain the
-- exact spelling; no file is renamed and no waiting caller changes lock inode.
ALTER TABLE agent_sessions ADD COLUMN fence_locator TEXT;
UPDATE agent_sessions SET fence_locator=id;
CREATE TRIGGER session_fence_locator_insert AFTER INSERT ON agent_sessions
WHEN NEW.fence_locator IS NULL
BEGIN
    UPDATE agent_sessions SET fence_locator=NEW.id WHERE id=NEW.id;
END;
CREATE TRIGGER session_fence_locator_immutable BEFORE UPDATE OF fence_locator ON agent_sessions
WHEN OLD.fence_locator IS NOT NULL AND NEW.fence_locator IS NOT OLD.fence_locator
BEGIN
    SELECT RAISE(ABORT, 'Session fence locator is immutable');
END;

ALTER TABLE processes ADD COLUMN receipt_locator TEXT;
ALTER TABLE processes ADD COLUMN journal_trace_locator TEXT;
UPDATE processes SET receipt_locator=id,journal_trace_locator=trace_id;
CREATE UNIQUE INDEX processes_receipt_locator ON processes(receipt_locator);
CREATE TRIGGER process_locators_insert AFTER INSERT ON processes
BEGIN
    UPDATE processes SET receipt_locator=COALESCE(NEW.receipt_locator,NEW.id),
        journal_trace_locator=COALESCE(NEW.journal_trace_locator,
            (SELECT journal_trace_locator FROM processes WHERE trace_id=NEW.trace_id
             AND id!=NEW.id ORDER BY started_at,id LIMIT 1),NEW.trace_id)
    WHERE id=NEW.id;
END;
CREATE TRIGGER process_locators_immutable BEFORE UPDATE OF receipt_locator,journal_trace_locator ON processes
WHEN (OLD.receipt_locator IS NOT NULL AND NEW.receipt_locator IS NOT OLD.receipt_locator)
    OR (OLD.journal_trace_locator IS NOT NULL AND NEW.journal_trace_locator IS NOT OLD.journal_trace_locator)
BEGIN
    SELECT RAISE(ABORT, 'Process filesystem locators are immutable');
END;

-- A stale waiter may still hold the original inode after commit. It must re-read
-- attachment authority, and cannot recreate an owner using its old spelling.
-- Released Session identities are bare hex or session_<hex>; other historical
-- Session names remain opaque.
CREATE TRIGGER canonical_session_id_insert BEFORE INSERT ON agent_sessions
WHEN NEW.id != lower(NEW.id) AND (
    (length(NEW.id)=32 AND NEW.id NOT GLOB '*[^0-9a-fA-F]*') OR
    (length(NEW.id)=40 AND lower(substr(NEW.id,1,8))='session_'
        AND substr(NEW.id,9) NOT GLOB '*[^0-9a-fA-F]*'))
BEGIN
    SELECT RAISE(ABORT, 'Session identity must be canonical');
END;
CREATE TRIGGER canonical_session_id_update BEFORE UPDATE OF id ON agent_sessions
WHEN NEW.id != lower(NEW.id) AND (
    (length(NEW.id)=32 AND NEW.id NOT GLOB '*[^0-9a-fA-F]*') OR
    (length(NEW.id)=40 AND lower(substr(NEW.id,1,8))='session_'
        AND substr(NEW.id,9) NOT GLOB '*[^0-9a-fA-F]*'))
BEGIN
    SELECT RAISE(ABORT, 'Session identity must be canonical');
END;
