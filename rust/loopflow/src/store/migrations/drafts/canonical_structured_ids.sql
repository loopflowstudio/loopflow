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

-- Canonical Wave associations do not rewrite content-addressed metric payloads.
CREATE TEMP TABLE canonical_waves (original TEXT PRIMARY KEY, canonical TEXT NOT NULL);
CREATE TEMP TRIGGER canonical_wave_collision BEFORE INSERT ON canonical_waves
WHEN EXISTS(SELECT 1 FROM waves WHERE id=NEW.canonical AND id!=NEW.original)
    OR EXISTS(SELECT 1 FROM canonical_waves WHERE canonical=NEW.canonical)
BEGIN SELECT RAISE(ABORT, 'canonical identity collision: Wave: ' || NEW.original || ' / ' || NEW.canonical); END;
INSERT INTO canonical_waves
WITH identities AS (
    SELECT id, CASE
        WHEN length(id)=38 AND substr(id,1,1)='{' AND substr(id,-1)='}'
            THEN substr(id,2,36)
        WHEN length(id)=45 AND lower(substr(id,1,9))='urn:uuid:'
            THEN substr(id,10)
        ELSE id END AS uuid
    FROM waves WHERE id!=lower(id)
)
SELECT id,lower(id) FROM identities WHERE
    (length(id)=32 AND id NOT GLOB '*[^0-9a-fA-F]*') OR
    (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-'
     AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-'
     AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*');
DROP TRIGGER preserve_capture_update;
DROP TRIGGER validate_conversation_ancestry_update;
DROP TRIGGER validate_project_conversation_parents;
DROP TRIGGER selected_project_owner_update;
UPDATE waves SET id=(SELECT canonical FROM canonical_waves WHERE original=waves.id)
WHERE id IN (SELECT original FROM canonical_waves);
UPDATE waves SET parent_wave_id=(SELECT canonical FROM canonical_waves WHERE original=waves.parent_wave_id)
WHERE parent_wave_id IN (SELECT original FROM canonical_waves);
UPDATE waves SET superseded_by_wave_id=(SELECT canonical FROM canonical_waves WHERE original=waves.superseded_by_wave_id)
WHERE superseded_by_wave_id IN (SELECT original FROM canonical_waves);
UPDATE projects SET wave_id=(SELECT canonical FROM canonical_waves WHERE original=projects.wave_id)
WHERE wave_id IN (SELECT original FROM canonical_waves);
UPDATE agent_sessions SET wave_id=(SELECT canonical FROM canonical_waves WHERE original=agent_sessions.wave_id)
WHERE wave_id IN (SELECT original FROM canonical_waves);
UPDATE session_events SET wave_id=(SELECT canonical FROM canonical_waves WHERE original=session_events.wave_id)
WHERE wave_id IN (SELECT original FROM canonical_waves);
UPDATE task_deletions SET wave_id=(SELECT canonical FROM canonical_waves WHERE original=task_deletions.wave_id)
WHERE wave_id IN (SELECT original FROM canonical_waves);
UPDATE task_issue_identities SET wave_id=(SELECT canonical FROM canonical_waves WHERE original=task_issue_identities.wave_id)
WHERE wave_id IN (SELECT original FROM canonical_waves);
UPDATE pm_wave_sync SET wave_id=(SELECT canonical FROM canonical_waves WHERE original=pm_wave_sync.wave_id)
WHERE wave_id IN (SELECT original FROM canonical_waves);
UPDATE pm_wave_projects SET wave_id=(SELECT canonical FROM canonical_waves WHERE original=pm_wave_projects.wave_id)
WHERE wave_id IN (SELECT original FROM canonical_waves);
UPDATE project_binding_imports SET wave_id=(SELECT canonical FROM canonical_waves WHERE original=project_binding_imports.wave_id)
WHERE wave_id IN (SELECT original FROM canonical_waves);
UPDATE wave_workflows SET wave_id=(SELECT canonical FROM canonical_waves WHERE original=wave_workflows.wave_id)
WHERE wave_id IN (SELECT original FROM canonical_waves);
UPDATE project_transitions SET wave_id=(SELECT canonical FROM canonical_waves WHERE original=project_transitions.wave_id)
WHERE wave_id IN (SELECT original FROM canonical_waves);
UPDATE project_transition_items SET wave_id=(SELECT canonical FROM canonical_waves WHERE original=project_transition_items.wave_id)
WHERE wave_id IN (SELECT original FROM canonical_waves);
UPDATE steers SET wave_id=(SELECT canonical FROM canonical_waves WHERE original=steers.wave_id)
WHERE wave_id IN (SELECT original FROM canonical_waves);
UPDATE work_placements SET wave_id=(SELECT canonical FROM canonical_waves WHERE original=work_placements.wave_id)
WHERE wave_id IN (SELECT original FROM canonical_waves);
UPDATE metric_instruments SET wave_id=(SELECT canonical FROM canonical_waves WHERE original=metric_instruments.wave_id)
WHERE wave_id IN (SELECT original FROM canonical_waves);
UPDATE metric_observations SET wave_id=(SELECT canonical FROM canonical_waves WHERE original=metric_observations.wave_id)
WHERE wave_id IN (SELECT original FROM canonical_waves);
UPDATE tool_responses SET work_id=(SELECT canonical FROM canonical_waves WHERE original=tool_responses.work_id)
WHERE work_kind='wave' AND work_id IN (SELECT original FROM canonical_waves);
CREATE TRIGGER preserve_capture_update BEFORE UPDATE ON session_events WHEN OLD.kind='captured'
BEGIN SELECT RAISE(ABORT,'Captured Session evidence is immutable'); END;
CREATE TRIGGER validate_conversation_ancestry_update
BEFORE UPDATE OF task_id,wave_id ON agent_sessions BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id=NEW.wave_id
    ) THEN RAISE(ABORT,'AgentSession Task and Wave disagree') END;
    SELECT CASE WHEN OLD.task_id IS NOT NULL AND NEW.task_id IS NOT OLD.task_id
        THEN RAISE(ABORT,'AgentSession binding is permanent') END;
    SELECT CASE WHEN OLD.wave_id IS NOT NULL AND NEW.wave_id IS NOT OLD.wave_id
        THEN RAISE(ABORT,'AgentSession Wave is permanent') END;
END;
CREATE TRIGGER validate_project_conversation_parents BEFORE UPDATE OF wave_id ON projects
WHEN EXISTS(SELECT 1 FROM agent_sessions s JOIN tasks t ON t.id=s.task_id
    WHERE t.project_id=NEW.id AND s.wave_id IS NOT NEW.wave_id)
BEGIN SELECT RAISE(ABORT,'Project would change AgentSession ancestry'); END;
CREATE TRIGGER selected_project_owner_update BEFORE UPDATE OF wave_id ON projects
WHEN EXISTS(SELECT 1 FROM waves WHERE current_project_id=NEW.id AND id!=NEW.wave_id)
BEGIN SELECT RAISE(ABORT, 'selected Project cannot change Wave ownership'); END;
DROP TRIGGER canonical_wave_collision;
DROP TABLE canonical_waves;
