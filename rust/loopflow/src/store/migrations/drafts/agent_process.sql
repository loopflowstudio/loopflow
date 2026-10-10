-- One process inventory. Backfill directly from the released Session shape;
-- history payloads and native thread identity remain unchanged.
ALTER TABLE processes ADD COLUMN kind TEXT NOT NULL DEFAULT 'lf' CHECK(kind IN ('lf','agent'));
ALTER TABLE processes ADD COLUMN agent_session_id TEXT REFERENCES agent_sessions(id);
ALTER TABLE processes ADD COLUMN os_started_at INTEGER;
ALTER TABLE processes ADD COLUMN endpoint TEXT;
ALTER TABLE processes ADD COLUMN agent_provider TEXT;
ALTER TABLE processes ADD COLUMN agent_interactive INTEGER CHECK(agent_interactive IN (0,1));
ALTER TABLE processes ADD COLUMN caller_agent_process_lfid TEXT;
ALTER TABLE processes ADD COLUMN attached_process_lfid TEXT;
ALTER TABLE processes ADD COLUMN attachment_token TEXT;
ALTER TABLE processes ADD COLUMN attachment_exit_seq INTEGER REFERENCES session_events(seq);
ALTER TABLE processes ADD COLUMN spawn_state TEXT CHECK(spawn_state IN ('reserved','spawn_requested','spawn_failed','exited'));
ALTER TABLE agent_sessions ADD COLUMN agent_process_lfid TEXT REFERENCES processes(lfid);
CREATE TEMP TABLE agent_process_backfill AS SELECT id AS session_id,
    (lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))),2) || '-8' ||
    substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6)))) AS lfid,
    CASE WHEN driver_generation>0 THEN
    (lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))),2) || '-8' ||
    substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6)))) END AS attachment_token
FROM agent_sessions WHERE provider_generation>0 OR provider_pid IS NOT NULL
    OR driver_generation>0 OR driver_process_lfid IS NOT NULL;
INSERT INTO processes(lfid,kind,trace_id,parent_process_lfid,command,repo,cwd,started_at,
    pid,os_started_at,agent_session_id,endpoint,agent_provider,agent_interactive,
    attached_process_lfid,attachment_token,attachment_exit_seq,spawn_state,completed_at)
SELECT b.lfid,'agent',COALESCE(p.trace_id,b.lfid),s.provider_process_lfid,
    s.provider,s.repo,s.cwd,COALESCE(s.provider_started_at,s.created_at),
    s.provider_pid,s.provider_started_at,s.id,s.provider_endpoint,s.provider,s.interactive,
    s.driver_process_lfid,b.attachment_token,
    (SELECT seq FROM session_events e WHERE e.session_id=s.id AND e.kind='observed'
        AND e.receipt_key='driver:'||(s.driver_generation-1)||':exit'),
    (SELECT json_extract(e.payload,'$.phase') FROM session_events e
     WHERE e.session_id=s.id AND e.receipt_key IN (
        'provider:'||s.provider_generation||':reserved',
        'provider:'||s.provider_generation||':spawn_requested',
        'provider:'||s.provider_generation||':spawn_failed',
        'provider:'||s.provider_generation||':exited') ORDER BY e.seq DESC LIMIT 1),
    (SELECT e.observed_at FROM session_events e WHERE e.session_id=s.id AND e.receipt_key IN (
        'provider:'||s.provider_generation||':spawn_failed',
        'provider:'||s.provider_generation||':exited') ORDER BY e.seq DESC LIMIT 1)
FROM agent_sessions s JOIN agent_process_backfill b ON b.session_id=s.id
LEFT JOIN processes p ON p.lfid=s.provider_process_lfid;
UPDATE agent_sessions SET agent_process_lfid=(SELECT lfid FROM agent_process_backfill b WHERE b.session_id=agent_sessions.id);
ALTER TABLE session_activity ADD COLUMN attachment_token TEXT;
UPDATE session_activity SET attachment_token=(
    SELECT b.attachment_token FROM agent_process_backfill b JOIN agent_sessions s ON s.id=b.session_id
    WHERE s.id=session_activity.session_id AND s.driver_generation=session_activity.driver_generation
);
-- The AgentProcess identity replaces the numeric provider generation. Only a
-- Session's current generation has a record; earlier ones stay unknown. The
-- released generation columns on processes and session_events remain as
-- unread history: nothing maps them, and dropping one rewrites every event.
ALTER TABLE session_activity ADD COLUMN agent_process_lfid TEXT;
UPDATE session_activity SET agent_process_lfid=(
    SELECT b.lfid FROM agent_process_backfill b JOIN agent_sessions s ON s.id=b.session_id
    WHERE s.id=session_activity.session_id AND s.provider_generation=session_activity.provider_generation
);
ALTER TABLE session_events ADD COLUMN agent_process_lfid TEXT;
UPDATE session_events SET agent_process_lfid=(
    SELECT b.lfid FROM agent_process_backfill b JOIN agent_sessions s ON s.id=b.session_id
    WHERE s.id=session_events.session_id AND s.provider_generation=session_events.provider_generation
) WHERE kind='started' AND provider_generation IS NOT NULL
    AND session_id IN (SELECT session_id FROM agent_process_backfill);
UPDATE processes SET caller_agent_process_lfid=(
    SELECT b.lfid FROM agent_process_backfill b JOIN agent_sessions s ON s.id=b.session_id
    WHERE s.id=processes.caller_session_id AND s.provider_generation=processes.caller_provider_generation
) WHERE caller_provider_generation IS NOT NULL;
DROP TABLE agent_process_backfill;
-- Preserve conflicting PID/birth observations separately; never deduplicate them
-- into signal authority or invent terminal evidence for uncertain history.
CREATE INDEX agent_process_session ON processes(agent_session_id) WHERE kind='agent';
CREATE INDEX agent_process_attachment ON processes(attached_process_lfid) WHERE kind='agent';
DROP INDEX session_driver_process;
DROP INDEX session_provider_pid;
DROP INDEX session_unknown_engine_origin;
DROP INDEX session_driver_exit;
CREATE INDEX session_observation_receipt ON session_events(session_id,receipt_key) WHERE kind='observed';
DROP TRIGGER store_revision_session_activity_update;
ALTER TABLE session_activity DROP COLUMN driver_generation;
ALTER TABLE session_activity DROP COLUMN provider_generation;
ALTER TABLE agent_sessions DROP COLUMN driver_generation;
ALTER TABLE agent_sessions DROP COLUMN driver_process_lfid;
ALTER TABLE agent_sessions DROP COLUMN provider_pid;
ALTER TABLE agent_sessions DROP COLUMN provider_started_at;
ALTER TABLE agent_sessions DROP COLUMN provider_endpoint;
ALTER TABLE agent_sessions DROP COLUMN provider_generation;
ALTER TABLE agent_sessions DROP COLUMN provider_process_lfid;
CREATE TRIGGER store_revision_session_activity_update AFTER UPDATE ON session_activity
WHEN NEW.attachment_token IS NOT OLD.attachment_token
    OR NEW.agent_process_lfid IS NOT OLD.agent_process_lfid
    OR NEW.program_status IS NOT OLD.program_status
    OR (NEW.pending_input > 0) IS NOT (OLD.pending_input > 0)
    OR (NEW.open_tools = 0) IS NOT (OLD.open_tools = 0)
    OR NEW.yielded IS NOT OLD.yielded
    OR (OLD.open_tools = 0 AND NEW.observed_at - OLD.observed_at >= 120)
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_agent_process_update AFTER UPDATE ON processes
WHEN NEW.kind='agent'
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
