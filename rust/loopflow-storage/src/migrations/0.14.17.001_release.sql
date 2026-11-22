-- draft: scope_tasks_capacity
-- A Wave's or repository's new Tasks may take a machine from a pool or an
-- on-demand (one namespace) instead of one named Machine. A scope sets one or
-- the other; the nearest scope that sets either wins.
ALTER TABLE scope_defaults ADD COLUMN tasks_capacity TEXT
    CHECK (tasks_capacity IS NULL OR tasks_machine_id IS NULL);

-- 0.14.16 rented RunPod pods from a template naming an image and recorded
-- hardware as a tagged CPU flavor or GPU; no current reader parses either, so
-- those rows go. Their pods' guardians already end them at their deadlines.
DELETE FROM rentals WHERE provider='runpod' AND json_extract(hardware,'$.kind') IS NOT NULL;
DELETE FROM ondemands WHERE provider='runpod' AND json_extract(template,'$.image') IS NOT NULL;

-- draft: rental_spot
-- Hardware says whether it is interruptible (spot); everything rented before
-- this was not.
UPDATE ondemands SET template = json_set(template, '$.spot', json('false'));
UPDATE rentals SET hardware = json_set(hardware, '$.spot', json('false'));

-- draft: session_activity_cpu
-- The launching process's CPU sample of its provider body, every 15 s: when it
-- last sampled and since when the body and its tools used no CPU. Updated in
-- place; store_revision_session_activity_update does not list these columns,
-- so samples never advance the `sessions` revision.
ALTER TABLE session_activity ADD COLUMN cpu_sampled_at INTEGER;
ALTER TABLE session_activity ADD COLUMN cpu_quiet_since INTEGER;

-- draft: session_facts_only
-- Session history keeps facts only, with one usage record per provider turn.
-- Captures stop writing stream events to the store, and native `usage` rows
-- become the only usage. This converts every store once: stream usage folds
-- into its native turn, transcript rows go, and the revision triggers stop
-- naming transcript shapes.

-- Every revision trigger is replaced below; dropping them first keeps the
-- per-row trigger out of the bulk delete.
DROP TRIGGER store_revision_session_events_insert;
DROP TRIGGER store_revision_session_events_update;
DROP TRIGGER store_revision_session_events_delete;
DROP TRIGGER store_revision_usage_insert;

-- Each stream usage observation, placed on the native turn its attempt's
-- provider session names. One observation per turn speaks for it: the final
-- receipt, else the latest.
CREATE TEMP TABLE facts_attempt_thread AS
SELECT captured_event,
       json_extract(payload,'$.evidence.attempt_key') AS attempt_key,
       json_extract(payload,'$.evidence.provider_session_id') AS thread,
       max(seq) AS seq
FROM session_events
WHERE kind='observed' AND instr(receipt_key,':events.jsonl:')>0 AND json_valid(payload)
  AND json_extract(payload,'$.evidence.type')='provider_session_observed'
GROUP BY captured_event, attempt_key;

CREATE TEMP TABLE facts_stream_usage AS
SELECT session_id, thread, turn, provider, usage, final_receipt, observed_at, captured_event
FROM (
    SELECT e.session_id, t.thread,
           json_extract(e.payload,'$.evidence.turn_key') AS turn,
           json_extract(e.payload,'$.evidence.provider') AS provider,
           json_extract(e.payload,'$.evidence.usage') AS usage,
           COALESCE(json_extract(e.payload,'$.evidence.final_receipt'),0) AS final_receipt,
           e.observed_at, e.captured_event,
           ROW_NUMBER() OVER (
               PARTITION BY e.session_id, t.thread, json_extract(e.payload,'$.evidence.turn_key')
               ORDER BY COALESCE(json_extract(e.payload,'$.evidence.final_receipt'),0) DESC,
                        json_extract(e.payload,'$.evidence.observation_seq') DESC, e.seq DESC) AS rank
    FROM session_events e
    JOIN facts_attempt_thread t ON t.captured_event=e.captured_event
        AND t.attempt_key=json_extract(e.payload,'$.evidence.attempt_key')
    WHERE e.kind='observed' AND instr(e.receipt_key,':events.jsonl:')>0 AND json_valid(e.payload)
      AND json_extract(e.payload,'$.evidence.type')='usage'
)
WHERE rank=1 AND thread IS NOT NULL AND turn IS NOT NULL;

-- 1. A Claude native receipt with a stream copy takes the copy's usage, which
--    carries cost, model and context pressure.
UPDATE session_events SET payload=json_object(
        'provider','claude','usage',json(s.usage),
        'final',json(CASE WHEN s.final_receipt THEN 'true' ELSE 'false' END))
FROM facts_stream_usage s
WHERE session_events.kind='usage' AND session_events.session_id=s.session_id
  AND session_events.provider_thread=s.thread AND session_events.provider_turn=s.turn
  AND json_valid(session_events.payload)
  AND json_extract(session_events.payload,'$.provider')='claude';

-- 2. Any other Claude native receipt is Claude's raw result usage; its cost
--    and model stay unknown.
UPDATE session_events SET payload=json_object('provider','claude','final',json('true'),'usage',json_object(
        'input_tokens',json_extract(payload,'$.usage.input_tokens'),
        'output_tokens',json_extract(payload,'$.usage.output_tokens'),
        'total_input_tokens',json_extract(payload,'$.usage.input_tokens')
            +COALESCE(json_extract(payload,'$.usage.cache_read_input_tokens'),0)
            +COALESCE(json_extract(payload,'$.usage.cache_creation_input_tokens'),0),
        'cache_read_tokens',json_extract(payload,'$.usage.cache_read_input_tokens'),
        'cache_write_tokens',json_extract(payload,'$.usage.cache_creation_input_tokens')))
WHERE kind='usage' AND json_valid(payload) AND json_extract(payload,'$.provider')='claude'
  AND json_type(payload,'$.final') IS NULL;

-- 3. A turn whose usage only the stream recorded gets its native row, kept
--    with the capture that observed it.
INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,observed_at,payload,captured_event)
SELECT s.session_id, s.thread, s.turn, 'usage', '', s.observed_at,
       json_object('provider',s.provider,'usage',json(s.usage),
           'final',json(CASE WHEN s.final_receipt THEN 'true' ELSE 'false' END)),
       s.captured_event
FROM facts_stream_usage s
WHERE NOT EXISTS(SELECT 1 FROM session_events n WHERE n.session_id=s.session_id
    AND n.provider_thread=s.thread AND n.provider_turn=s.turn AND n.kind='usage');

-- 4. Captures recorded before history kept facts only: keep the silences
--    their stream implied, then remove everything that is not a fact.
CREATE TEMP TABLE facts_stream AS
SELECT seq, session_id, task_id, wave_id, observed_at, captured_event,
       substr(receipt_key,1,instr(receipt_key,':events.jsonl:')-1) AS input,
       CAST(substr(receipt_key,instr(receipt_key,':events.jsonl:')+14) AS INTEGER) AS ordinal,
       CASE WHEN json_valid(payload) THEN
           json_extract(payload,'$.evidence.type') IN ('provider_attempt_started',
               'provider_attempt_finished','provider_account_selected','provider_session_observed')
           OR (json_extract(payload,'$.evidence.type')='conversation'
               AND json_extract(payload,'$.evidence.event.type') IN ('turn_completed','error'))
       ELSE 0 END AS fact
FROM session_events
WHERE kind='observed' AND instr(receipt_key,':events.jsonl:')>0;

INSERT OR IGNORE INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload,captured_event)
SELECT session_id,'observed',input||':silence:'||ordinal,task_id,wave_id,observed_at,
       json_object('input_id',input,'source','silence:'||ordinal,
           'evidence',json_object('since',since,'observed_at',observed_at)),captured_event
FROM (SELECT *, LAG(observed_at) OVER (PARTITION BY captured_event ORDER BY ordinal,seq) AS since
      FROM facts_stream
      WHERE captured_event IN (SELECT captured_event FROM facts_stream WHERE NOT fact))
WHERE observed_at-since>600;

DELETE FROM session_events WHERE seq IN (SELECT seq FROM facts_stream WHERE NOT fact);

-- Replay reads the prompt from the capture's own manifest file.
UPDATE session_events SET payload=json_set(payload,'$.evidence.exec',json('null'))
WHERE kind='observed' AND substr(receipt_key,-14)=':manifest.json'
  AND json_valid(payload) AND json_extract(payload,'$.evidence.exec') IS NOT NULL;

DROP TABLE facts_attempt_thread;
DROP TABLE facts_stream_usage;
DROP TABLE facts_stream;

-- 5. History holds facts, so every row but usage is a Session change.
CREATE TRIGGER store_revision_session_events_insert AFTER INSERT ON session_events
WHEN NEW.kind != 'usage'
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_session_events_update AFTER UPDATE ON session_events
WHEN NEW.kind != 'usage' OR OLD.kind != 'usage'
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_session_events_delete AFTER DELETE ON session_events
WHEN OLD.kind != 'usage'
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;

-- usage: one native row per provider turn, replaced as receipts arrive. A
-- reader showing totals follows this domain alone.
CREATE TRIGGER store_revision_usage_insert AFTER INSERT ON session_events
WHEN NEW.kind = 'usage'
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'usage';
END;

UPDATE store_revisions SET revision = revision + 1 WHERE domain IN ('sessions', 'usage');

-- draft: ship_it
-- A Task's Workflow is a graph. Captured rows at `end` keep their position with
-- an empty graph; every other row is removed, and the next `lf task run` takes up
-- the default graph placed at the Task's last recorded stage. The graph column is
-- fixed by trigger, so the trigger steps aside for the rewrite.
DROP TRIGGER task_workflow_graph_is_fixed;
DELETE FROM task_workflows WHERE NOT (node='end' AND edge IS NULL);
UPDATE task_workflows SET graph=json_object('name',json_extract(graph,'$.name'),'steps',json('[]'));
CREATE TRIGGER task_workflow_graph_is_fixed BEFORE UPDATE OF graph ON task_workflows BEGIN
    SELECT RAISE(ABORT,'A Workflow graph is fixed when taken up');
END;

-- Builtin stages are named by their skills: the `design` and `findings` nodes of the
-- earlier definitions are `explore` and `research`. The history table is
-- append-only, so it is rebuilt with its rows, index and triggers.
CREATE TABLE task_workflow_moves_renamed (
    seq INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    workflow TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('took_up', 'chose', 'arrived', 'set')),
    from_node TEXT NOT NULL,
    to_node TEXT NOT NULL,
    edge INTEGER,
    "lf_process_id" TEXT REFERENCES "processes"("id"),
    note TEXT,
    at INTEGER NOT NULL
) STRICT;
INSERT INTO task_workflow_moves_renamed(seq,task_id,workflow,kind,from_node,to_node,edge,lf_process_id,note,at)
SELECT seq,task_id,workflow,kind,
    CASE from_node WHEN 'design' THEN 'explore' WHEN 'findings' THEN 'research' ELSE from_node END,
    CASE to_node WHEN 'design' THEN 'explore' WHEN 'findings' THEN 'research' ELSE to_node END,
    edge,lf_process_id,note,at FROM task_workflow_moves;
DROP TABLE task_workflow_moves;
ALTER TABLE task_workflow_moves_renamed RENAME TO task_workflow_moves;
CREATE INDEX task_workflow_moves_task ON task_workflow_moves(task_id, seq);
CREATE TRIGGER store_revision_task_workflow_moves_insert AFTER INSERT ON task_workflow_moves
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_workflow_moves_update AFTER UPDATE ON task_workflow_moves
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_workflow_moves_delete AFTER DELETE ON task_workflow_moves
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER task_workflow_moves_are_append_only BEFORE UPDATE ON task_workflow_moves BEGIN
    SELECT RAISE(ABORT,'Workflow history is append-only');
END;

-- Shipping is a Task lifecycle state: `shipped_at` is when the Task was handed off
-- to finish on its own (shared, like `paused`), and `ship_lf_process_id` is this
-- Machine's process carrying it there. `ship_restarts` counts restarts in a row
-- after a failed ship process; each waits longer, and a completed step resets it.
ALTER TABLE tasks ADD COLUMN shipped_at INTEGER;
ALTER TABLE tasks ADD COLUMN ship_lf_process_id TEXT REFERENCES processes(id);
ALTER TABLE tasks ADD COLUMN ship_restarts INTEGER NOT NULL DEFAULT 0;

-- CI repair has no per-Task setting: Defer holds it, and the ship process is the
-- only repair starter.
ALTER TABLE tasks DROP COLUMN automation_enabled;

-- No command records a pending abandonment: abandon ends the Task in one write,
-- keeping its reason in `tasks.abandon_reason`.
ALTER TABLE tasks DROP COLUMN abandon_requested_at;
ALTER TABLE projects DROP COLUMN abandon_requested_at;
ALTER TABLE projects DROP COLUMN abandon_reason;
