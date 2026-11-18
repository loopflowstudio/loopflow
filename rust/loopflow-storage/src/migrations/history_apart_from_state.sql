-- A viewer heartbeat is a reading time, not a change. The holder re-stamps
-- `viewer_hold_at` every ten seconds per viewed terminal; counting each stamp
-- woke every subscription about five times a second. Retirement reads the
-- stamp on its own clock, so only its first value counts, beside every other
-- column.
DROP TRIGGER store_revision_held_terminals_update;
CREATE TRIGGER store_revision_held_terminals_update AFTER UPDATE ON held_terminals
WHEN NEW.id IS NOT OLD.id OR NEW.holder_build IS NOT OLD.holder_build OR
    NEW.created_at IS NOT OLD.created_at OR NEW.ended_at IS NOT OLD.ended_at OR
    NEW.exit_code IS NOT OLD.exit_code OR NEW.viewer_hold IS NOT OLD.viewer_hold OR
    NEW.viewed IS NOT OLD.viewed OR
    (NEW.viewer_hold_at IS NULL) IS NOT (OLD.viewer_hold_at IS NULL)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;

-- Nothing writes or reads `task_update` observations since their owner was
-- deleted; only Graph Session facts still move Task updates.
DROP TRIGGER task_update_session_events_insert;
CREATE TRIGGER task_update_session_events_insert AFTER INSERT ON session_events
WHEN EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.lf_process_id)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

DROP TRIGGER task_update_session_events_update;
CREATE TRIGGER task_update_session_events_update AFTER UPDATE ON session_events
WHEN EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.lf_process_id)
    OR EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.lf_process_id)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

DROP TRIGGER task_update_session_events_delete;
CREATE TRIGGER task_update_session_events_delete AFTER DELETE ON session_events
WHEN EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.lf_process_id)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

-- Which conversation `lf resume` reopens is state: the latest time a person
-- opened it, in milliseconds. It was an `interactive_opened` observation per
-- opening; the history file keeps no copy.
ALTER TABLE agent_sessions ADD COLUMN opened_at INTEGER;
UPDATE agent_sessions SET opened_at=(
    SELECT MAX(json_extract(payload,'$.opened_at_ms')) FROM session_events
    WHERE session_id=agent_sessions.id AND kind='observed'
      AND json_valid(payload) AND json_extract(payload,'$.type')='interactive_opened')
WHERE interactive=1;

-- What resumes an input and how it was launched are state of that input, so
-- they live on its `captured` row: the provider conversation and account its
-- newest `provider-session:*` observation named, and whether its manifest ran
-- independently of any Graph. A captured row stays immutable otherwise.
ALTER TABLE session_events ADD COLUMN provider_account TEXT;
ALTER TABLE session_events ADD COLUMN independent INTEGER;
DROP TRIGGER preserve_capture_update;
CREATE TRIGGER preserve_capture_update BEFORE UPDATE ON session_events WHEN OLD.kind='captured'
    AND (NEW.seq IS NOT OLD.seq OR NEW.session_id IS NOT OLD.session_id OR NEW.kind IS NOT OLD.kind
    OR NEW.provider_turn IS NOT OLD.provider_turn OR NEW.receipt_key IS NOT OLD.receipt_key
    OR NEW.provider_generation IS NOT OLD.provider_generation OR NEW.lf_process_id IS NOT OLD.lf_process_id
    OR NEW.task_id IS NOT OLD.task_id OR NEW.wave_id IS NOT OLD.wave_id
    OR NEW.observed_at IS NOT OLD.observed_at OR NEW.payload IS NOT OLD.payload
    OR NEW.captured_event IS NOT OLD.captured_event OR NEW.agent_process_id IS NOT OLD.agent_process_id)
BEGIN SELECT RAISE(ABORT,'Captured Session evidence is immutable'); END;
UPDATE session_events AS c SET
    provider_thread=json_extract(pref.payload,'$.evidence.provider_session_id'),
    provider_account=json_extract(pref.payload,'$.evidence.account_id')
FROM (SELECT o.session_id,substr(o.receipt_key,1,instr(o.receipt_key,':provider-session:')-1) AS input,
        o.payload,ROW_NUMBER() OVER (PARTITION BY o.session_id,
            substr(o.receipt_key,1,instr(o.receipt_key,':provider-session:')-1) ORDER BY o.seq DESC) AS newest
    FROM session_events o WHERE o.kind='observed' AND instr(o.receipt_key,':provider-session:')>0
      AND json_valid(o.payload) AND json_extract(o.payload,'$.evidence.schema_version')=1
      AND length(json_extract(o.payload,'$.evidence.provider_session_id'))>0) AS pref
WHERE c.kind='captured' AND c.session_id=pref.session_id AND c.receipt_key=pref.input AND pref.newest=1;
UPDATE session_events AS c SET independent=1
FROM session_events m
WHERE c.kind='captured' AND m.session_id=c.session_id AND m.kind='observed'
  AND m.receipt_key=c.receipt_key||':manifest.json' AND json_valid(m.payload)
  AND json_extract(m.payload,'$.evidence.flow.kind')='independent';

-- A Session's latest provider transition is state its summary shows; the list
-- of every transition is history.
ALTER TABLE agent_sessions ADD COLUMN transition TEXT;
UPDATE agent_sessions AS s SET transition=CASE
    WHEN json_extract(t.payload,'$.type')='provider_transition' THEN json_extract(t.payload,'$.transition')
    ELSE json_object('kind',CASE WHEN json_extract(t.payload,'$.source')='fork' THEN 'forked' ELSE 'started' END,
        'from_provider_session',json_extract(t.payload,'$.provider_thread'),
        'to_provider_session',json_extract(t.payload,'$.next_provider_thread'),
        'from_agent_process',CASE WHEN json_extract(t.payload,'$.provider_thread') IS NOT NULL
            THEN json_extract(t.payload,'$.agent_process_id') END,
        'to_agent_process',json_extract(t.payload,'$.agent_process_id'),
        'original_running',NULL,
        'seed',CASE WHEN json_extract(t.payload,'$.source')='fork' THEN 'native' END,
        'observed_at',t.observed_at) END
FROM (SELECT session_id,payload,observed_at,ROW_NUMBER() OVER (PARTITION BY session_id ORDER BY seq DESC) AS newest
    FROM session_events WHERE kind='observed'
      AND (receipt_key GLOB 'provider-transition:*' OR receipt_key GLOB 'agent-session:*')
      AND json_valid(payload)) AS t
WHERE t.session_id=s.id AND t.newest=1;
CREATE INDEX session_capture_provider_thread ON session_events(provider_thread)
    WHERE kind='captured' AND provider_thread IS NOT NULL;

-- Where a moved conversation went is state the next attachment reads. It is
-- keyed by the attachment that ended, so a later one never answers for it.
ALTER TABLE agent_sessions ADD COLUMN departed_attachment TEXT;
ALTER TABLE agent_sessions ADD COLUMN departure TEXT;
UPDATE agent_sessions AS s SET departed_attachment=d.token,departure=d.departure
FROM (SELECT session_id,json_extract(payload,'$.attachment_token') AS token,
        json_extract(payload,'$.departure') AS departure,
        ROW_NUMBER() OVER (PARTITION BY session_id ORDER BY seq DESC) AS newest
    FROM session_events WHERE kind='observed' AND receipt_key GLOB 'attachment:*:exit'
      AND json_valid(payload) AND json_extract(payload,'$.departure') IS NOT NULL) AS d
WHERE d.session_id=s.id AND d.newest=1;

-- A fork's lineage seeds the first transition of the input it launched: state
-- on the input's captured row until that transition takes it.
ALTER TABLE session_events ADD COLUMN provider_lineage TEXT;
UPDATE session_events AS c SET provider_lineage=l.payload
FROM (SELECT captured_event,payload,
        ROW_NUMBER() OVER (PARTITION BY captured_event ORDER BY seq DESC) AS newest
    FROM session_events l WHERE kind='observed' AND receipt_key GLOB 'lineage:*'
      AND captured_event IS NOT NULL AND json_valid(payload)
      AND NOT EXISTS(SELECT 1 FROM session_events t WHERE t.captured_event=l.captured_event
          AND t.kind='observed' AND t.receipt_key GLOB 'provider-transition:*')) AS l
WHERE c.kind='captured' AND c.seq=l.captured_event AND l.newest=1;

-- Turns live in history.db, whose own revision row counts them.
DROP TRIGGER store_revision_usage_insert;

-- A Task's work is its turns in history.db now; `started` rows in
-- session_events convert there and go, including a Task's last one.
DROP TRIGGER retain_task_agent_turn;

-- How an input's newest ended turn went is state: Graph routing reads the turn
-- that answered it, the Session list whether it ended, and a failed step
-- whether it was interrupted or errored. It lives on the input's captured row,
-- written in this file when the turn ends; history.db keeps the turn's numbers.
ALTER TABLE session_events ADD COLUMN turn_thread TEXT;
ALTER TABLE session_events ADD COLUMN turn_id TEXT;
ALTER TABLE session_events ADD COLUMN turn_ended_at INTEGER;
ALTER TABLE session_events ADD COLUMN turn_status TEXT;
ALTER TABLE session_events ADD COLUMN turn_error TEXT;
UPDATE session_events AS c SET turn_thread=e.provider_thread,turn_id=e.provider_turn,
    turn_ended_at=e.observed_at,turn_status=json_extract(e.payload,'$.status')
FROM (SELECT captured_event,provider_thread,provider_turn,observed_at,payload,
        ROW_NUMBER() OVER (PARTITION BY captured_event ORDER BY observed_at DESC,seq DESC) AS newest
    FROM session_events WHERE kind='completed' AND captured_event IS NOT NULL
      AND provider_thread IS NOT NULL AND provider_turn IS NOT NULL AND json_valid(payload)) AS e
WHERE c.kind='captured' AND c.seq=e.captured_event AND e.newest=1;
CREATE INDEX session_capture_turn ON session_events(session_id,turn_thread,turn_id)
    WHERE kind='captured' AND turn_id IS NOT NULL;
