-- name: capture_session_events
-- id: 76852b62431d481188c741fc4dd8999a
-- depends_on: record_session_output

-- Capture identity is a Session history sequence. Historical SQL remains exact
-- import evidence; no new reservation or continuation writes the archive.


DROP TRIGGER validate_conversation_ancestry_insert;

DROP TRIGGER validate_conversation_ancestry_update;

DROP TRIGGER validate_flow_conversation_parents;

DROP TRIGGER validate_task_conversation_parents;

DROP TRIGGER validate_project_conversation_parents;

DROP TRIGGER task_first_conversation_insert;

DROP TRIGGER task_first_conversation_bind;

DROP TRIGGER validate_flow_input;

DROP TRIGGER retain_imported_input;

DROP TRIGGER retain_historical_input;

DROP TRIGGER validate_flow_input_parents;

DROP TRIGGER validate_project_input_parents;

DROP TRIGGER validate_task_input_parents;

DROP TRIGGER task_first_agent_turn;

DROP TRIGGER retain_task_agent_turn;

DROP TRIGGER validate_task_started_update;

CREATE TABLE import_evidence (
    source TEXT NOT NULL,
    selector TEXT NOT NULL,
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    historical_task_id TEXT GENERATED ALWAYS AS (json_extract(payload,'$.task_id')) VIRTUAL REFERENCES tasks(id),
    historical_wave_id TEXT GENERATED ALWAYS AS (json_extract(payload,'$.wave_id')) VIRTUAL REFERENCES waves(id),
    historical_flow_id TEXT GENERATED ALWAYS AS (json_extract(payload,'$.invocation_id')) VIRTUAL REFERENCES flow_sessions(id),
    historical_session_id TEXT GENERATED ALWAYS AS (json_extract(payload,'$.session_id')) VIRTUAL REFERENCES agent_sessions(id),
    PRIMARY KEY(source,selector)
);
INSERT INTO import_evidence(source,selector,payload)
SELECT 'runs',input_id,imported_sql FROM agent_session_inputs WHERE imported_sql IS NOT NULL;
INSERT INTO import_evidence(source,selector,payload)
SELECT 'agent_session_inputs',input_id,json_object('input_id',input_id,'caller_input_id',caller_input_id)
FROM agent_session_inputs WHERE session_id IS NULL AND imported_sql IS NULL;
CREATE INDEX import_evidence_task ON import_evidence(historical_task_id);
CREATE INDEX import_evidence_flow ON import_evidence(historical_flow_id);

CREATE TABLE captured_history (
    seq INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES agent_sessions(id),
    provider_thread TEXT,
    provider_turn TEXT,
    kind TEXT NOT NULL CHECK(kind IN ('started','usage','completed','observed','output','captured')),
    receipt_key TEXT NOT NULL,
    provider_generation INTEGER,
    exec_id TEXT REFERENCES execs(id),
    task_id TEXT REFERENCES tasks(id),
    wave_id TEXT REFERENCES waves(id),
    observed_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    captured_event INTEGER REFERENCES session_events(seq)
);

INSERT INTO captured_history SELECT seq,session_id,provider_thread,provider_turn,kind,receipt_key,provider_generation,exec_id,task_id,wave_id,observed_at,payload,NULL FROM session_events;

CREATE TEMP TABLE native_input_refs AS SELECT seq,input_id FROM session_events;

DROP TABLE session_events;

ALTER TABLE captured_history RENAME TO session_events;

INSERT INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload)
SELECT i.session_id,'captured',i.input_id,
    COALESCE(i.historical_task_id,m.task_id,CASE WHEN s.input_id=i.input_id THEN s.task_id END),
    COALESCE(i.historical_wave_id,m.wave_id,CASE WHEN s.input_id=i.input_id THEN s.wave_id END),
    COALESCE(json_extract(i.imported_sql,'$.created_at'),m.observed_at,s.created_at),
    json_object('artifact_key',i.input_id,'caller_key',i.caller_input_id)
FROM agent_session_inputs i JOIN agent_sessions s ON s.id=i.session_id
LEFT JOIN session_events m ON m.session_id=i.session_id AND m.receipt_key=i.input_id||':manifest.json' AND m.kind='observed';
-- Some historical Session rows predate catalog admission.
INSERT INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload)
SELECT s.id,'captured',s.input_id,s.task_id,s.wave_id,s.created_at,json_object('artifact_key',s.input_id)
FROM agent_sessions s WHERE NOT EXISTS(SELECT 1 FROM session_events e WHERE e.kind='captured' AND e.receipt_key=s.input_id);
UPDATE session_events SET captured_event=(SELECT c.seq FROM session_events c JOIN native_input_refs n
    ON n.input_id=c.receipt_key WHERE n.seq=session_events.seq AND c.kind='captured');
UPDATE session_events SET captured_event=(SELECT c.seq FROM session_events c
    WHERE c.kind='captured' AND c.session_id=session_events.session_id
      AND substr(session_events.receipt_key,1,length(c.receipt_key)+1)=c.receipt_key||':')
WHERE kind='observed';
DROP TABLE native_input_refs;

CREATE TABLE captured_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    current_capture INTEGER UNIQUE REFERENCES session_events(seq),
    title TEXT NOT NULL,
    title_source TEXT NOT NULL CHECK (title_source IN ('generated', 'human')),
    ready_summary TEXT,
    completed_at INTEGER,
    created_at INTEGER NOT NULL,
    request TEXT,
    driver_exec_id TEXT REFERENCES execs(id),
    driver_generation INTEGER NOT NULL DEFAULT 0,
    provider_generation INTEGER NOT NULL DEFAULT 0,
    provider_exec_id TEXT REFERENCES execs(id),
    interactive INTEGER NOT NULL DEFAULT 1 CHECK (interactive IN (0, 1)),
    kind TEXT NOT NULL DEFAULT 'conversation' CHECK (kind IN ('conversation', 'flow_review', 'ask')),
    repo TEXT,
    provider_endpoint TEXT,
    provider_thread TEXT,
    provider_pid INTEGER,
    provider_started_at INTEGER,
    task_id TEXT REFERENCES tasks(id),
    wave_id TEXT REFERENCES waves(id),
    flow_session_id TEXT REFERENCES flow_sessions(id),
    work_source TEXT CHECK (work_source IN ('declared', 'checkout', 'inherited', 'bound')),
    bound_at INTEGER,
    input_published INTEGER NOT NULL CHECK(input_published IN (0,1)),
    cwd TEXT NOT NULL,
    skill TEXT,
    provider TEXT,
    model TEXT,
    node INTEGER,
    iterations TEXT
);

INSERT INTO captured_sessions(id,current_capture,title,title_source,ready_summary,completed_at,created_at,request,driver_exec_id,driver_generation,provider_generation,provider_exec_id,interactive,kind,repo,provider_endpoint,provider_thread,provider_pid,provider_started_at,task_id,wave_id,flow_session_id,work_source,bound_at,input_published,cwd,skill,provider,model,node,iterations) SELECT s.id,(SELECT e.seq FROM session_events e WHERE e.kind='captured' AND e.session_id=s.id AND e.receipt_key=s.input_id),s.title,s.title_source,s.ready_summary,s.completed_at,s.created_at,s.request,s.driver_exec_id,s.driver_generation,s.provider_generation,s.provider_exec_id,s.interactive,s.kind,s.repo,s.provider_endpoint,s.provider_thread,s.provider_pid,s.provider_started_at,s.task_id,s.wave_id,s.flow_session_id,s.work_source,s.bound_at,s.input_published,s.cwd,s.skill,s.provider,s.model,s.node,s.iterations FROM agent_sessions s;

DROP TABLE agent_sessions;

ALTER TABLE captured_sessions RENAME TO agent_sessions;

DROP INDEX flow_metadata;

-- Preserve selectors whose conversation membership could never be established.
INSERT INTO import_evidence(source,selector,payload)
SELECT 'flow_capture',id,json_object('current_run_id',current_run_id,'position_version',position_version)
FROM flow_sessions WHERE current_run_id IS NOT NULL;

ALTER TABLE flow_sessions ADD COLUMN current_capture INTEGER REFERENCES session_events(seq);

UPDATE flow_sessions SET current_capture=(SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=flow_sessions.current_run_id);

ALTER TABLE flow_sessions DROP COLUMN current_run_id;

-- Preserve exact prior failure attribution before replacing its selector with
-- the captured event, including failures with no established conversation.
INSERT INTO import_evidence(source,selector,payload)
SELECT 'flow_failure',id,failure_json FROM flow_sessions WHERE failure_json IS NOT NULL;
UPDATE flow_sessions SET failure_json=json_remove(json_set(failure_json,'$.captured',
    (SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=json_extract(flow_sessions.failure_json,'$.run_id'))),'$.run_id')
WHERE failure_json IS NOT NULL;

DROP TABLE agent_session_inputs;

CREATE INDEX open_sessions ON "agent_sessions"(created_at, id) WHERE completed_at IS NULL;

CREATE INDEX session_inventory ON "agent_sessions"(interactive, completed_at, title, id);

CREATE INDEX session_repo_inventory ON "agent_sessions"(repo, interactive, completed_at, title, id);

CREATE INDEX conversation_task_inventory ON agent_sessions(task_id,title,id);

CREATE INDEX session_driver_exec ON agent_sessions(driver_exec_id) WHERE driver_exec_id IS NOT NULL;

CREATE INDEX session_provider_pid ON agent_sessions(provider_pid) WHERE provider_pid IS NOT NULL;

CREATE INDEX session_unknown_engine_origin ON agent_sessions(provider_exec_id) WHERE provider_pid IS NULL;

CREATE INDEX flow_metadata ON flow_sessions(
    id, CASE WHEN json_valid(invocation_json) THEN CASE WHEN json_type(invocation_json,'$.flow')='text' THEN json_extract(invocation_json,'$.flow') END END, state, current_capture, pending_session_id,
    task_id, wave_id, updated_at);

CREATE UNIQUE INDEX session_event_receipt ON session_events(
    session_id, COALESCE(provider_thread,''), COALESCE(provider_turn,''), kind, receipt_key);

CREATE INDEX session_events_history ON session_events(session_id,seq);

CREATE INDEX session_events_turn ON session_events(session_id,provider_thread,provider_turn,seq);

CREATE INDEX session_event_input ON session_events(session_id,captured_event) WHERE kind='started';

CREATE INDEX session_input_membership ON session_events(
    session_id, captured_event, CASE WHEN json_valid(payload) THEN CASE WHEN json_extract(payload,'$.source')='manifest.json' AND json_extract(payload,'$.evidence.schema_version')=1 AND json_extract(payload,'$.evidence.run_id')=json_extract(payload,'$.input_id') AND receipt_key=json_extract(payload,'$.input_id')||':manifest.json' THEN json_extract(payload,'$.evidence.flow.kind') END END)
    WHERE kind='observed' AND substr(receipt_key,-14)=':manifest.json';

CREATE UNIQUE INDEX session_capture_key ON session_events(receipt_key) WHERE kind='captured';

CREATE TRIGGER preserve_import_evidence_update BEFORE UPDATE ON import_evidence
BEGIN SELECT RAISE(ABORT,'Historical SQL evidence is immutable'); END;
CREATE TRIGGER preserve_import_evidence_delete BEFORE DELETE ON import_evidence
BEGIN SELECT RAISE(ABORT,'Historical SQL evidence must be retained'); END;
CREATE TRIGGER preserve_capture_update BEFORE UPDATE ON session_events WHEN OLD.kind='captured'
BEGIN SELECT RAISE(ABORT,'Captured Session evidence is immutable'); END;
CREATE TRIGGER preserve_capture_delete BEFORE DELETE ON session_events WHEN OLD.kind='captured'
BEGIN SELECT RAISE(ABORT,'Captured Session evidence must be retained'); END;
CREATE TRIGGER validate_session_capture_insert BEFORE INSERT ON agent_sessions
WHEN NEW.current_capture IS NOT NULL AND NOT EXISTS(SELECT 1 FROM session_events WHERE seq=NEW.current_capture AND session_id=NEW.id AND kind='captured')
BEGIN SELECT RAISE(ABORT,'Captured event belongs to another Session'); END;
CREATE TRIGGER validate_session_capture BEFORE UPDATE OF current_capture ON agent_sessions
WHEN NEW.current_capture IS NOT NULL AND NOT EXISTS(SELECT 1 FROM session_events WHERE seq=NEW.current_capture AND session_id=NEW.id AND kind='captured')
BEGIN SELECT RAISE(ABORT,'Captured event belongs to another Session'); END;
CREATE TRIGGER validate_flow_capture BEFORE UPDATE OF current_capture ON flow_sessions
WHEN NEW.current_capture IS NOT NULL AND NOT EXISTS(SELECT 1 FROM agent_sessions WHERE current_capture=NEW.current_capture AND flow_session_id=NEW.id)
BEGIN SELECT RAISE(ABORT,'Flow capture belongs to another conversation'); END;

CREATE TRIGGER validate_conversation_ancestry_insert BEFORE INSERT ON agent_sessions BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id=NEW.wave_id
    ) THEN RAISE(ABORT,'AgentSession Task and Wave disagree') END;
    SELECT CASE WHEN NEW.flow_session_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM flow_sessions f WHERE f.id=NEW.flow_session_id
        AND f.task_id IS NEW.task_id AND (f.wave_id IS NULL OR f.wave_id IS NEW.wave_id)
    ) THEN RAISE(ABORT,'AgentSession and FlowSession ancestry disagree') END;
END;

CREATE TRIGGER validate_conversation_ancestry_update
BEFORE UPDATE OF task_id,wave_id,flow_session_id ON agent_sessions BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id=NEW.wave_id
    ) THEN RAISE(ABORT,'AgentSession Task and Wave disagree') END;
    SELECT CASE WHEN NEW.flow_session_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM flow_sessions f WHERE f.id=NEW.flow_session_id
        AND f.task_id IS NEW.task_id AND (f.wave_id IS NULL OR f.wave_id IS NEW.wave_id)
    ) THEN RAISE(ABORT,'AgentSession and FlowSession ancestry disagree') END;
    SELECT CASE WHEN OLD.task_id IS NOT NULL AND NEW.task_id IS NOT OLD.task_id
        THEN RAISE(ABORT,'AgentSession binding is permanent') END;
    SELECT CASE WHEN OLD.wave_id IS NOT NULL AND NEW.wave_id IS NOT OLD.wave_id
        THEN RAISE(ABORT,'AgentSession Wave is permanent') END;
    SELECT CASE WHEN OLD.flow_session_id IS NOT NULL AND NEW.flow_session_id IS NOT OLD.flow_session_id
        THEN RAISE(ABORT,'AgentSession Flow membership is permanent') END;
END;

CREATE TRIGGER validate_flow_conversation_parents BEFORE UPDATE OF task_id,wave_id ON flow_sessions
WHEN EXISTS(SELECT 1 FROM agent_sessions s WHERE s.flow_session_id=NEW.id
    AND (s.task_id IS NOT NEW.task_id OR (NEW.wave_id IS NOT NULL AND s.wave_id IS NOT NEW.wave_id)))
BEGIN SELECT RAISE(ABORT,'FlowSession would change AgentSession ancestry'); END;

CREATE TRIGGER validate_task_conversation_parents BEFORE UPDATE OF project_id ON tasks
WHEN EXISTS(SELECT 1 FROM agent_sessions s JOIN projects p ON p.id=NEW.project_id
    WHERE s.task_id=NEW.id AND s.wave_id IS NOT p.wave_id)
BEGIN SELECT RAISE(ABORT,'Task would change AgentSession ancestry'); END;

CREATE TRIGGER validate_project_conversation_parents BEFORE UPDATE OF wave_id ON projects
WHEN EXISTS(SELECT 1 FROM agent_sessions s JOIN tasks t ON t.id=s.task_id
    WHERE t.project_id=NEW.id AND s.wave_id IS NOT NEW.wave_id)
BEGIN SELECT RAISE(ABORT,'Project would change AgentSession ancestry'); END;

CREATE TRIGGER task_first_conversation_insert AFTER INSERT ON agent_sessions
WHEN NEW.task_id IS NOT NULL BEGIN
    UPDATE tasks SET started_at=coalesce(NEW.bound_at,CAST(strftime('%s','now') AS INTEGER))
    WHERE id=NEW.task_id AND started_at IS NULL;
END;

CREATE TRIGGER task_first_conversation_bind AFTER UPDATE OF task_id ON agent_sessions
WHEN OLD.task_id IS NULL AND NEW.task_id IS NOT NULL BEGIN
    UPDATE tasks SET started_at=coalesce(NEW.bound_at,CAST(strftime('%s','now') AS INTEGER))
    WHERE id=NEW.task_id AND started_at IS NULL;
END;

CREATE TRIGGER validate_flow_input_parents BEFORE UPDATE OF task_id ON "flow_sessions"
WHEN EXISTS (SELECT 1 FROM import_evidence WHERE historical_flow_id=NEW.id AND historical_task_id IS NOT NEW.task_id)
BEGIN SELECT RAISE(ABORT, 'FlowSession would change historical input ancestry'); END;

CREATE TRIGGER validate_project_input_parents BEFORE UPDATE OF wave_id ON projects
WHEN EXISTS (SELECT 1 FROM import_evidence r JOIN tasks t ON t.id=r.historical_task_id
    WHERE t.project_id=NEW.id AND r.historical_wave_id IS NOT NEW.wave_id)
BEGIN SELECT RAISE(ABORT, 'Project would change historical input ancestry'); END;

CREATE TRIGGER validate_task_input_parents BEFORE UPDATE OF project_id ON tasks
WHEN EXISTS (SELECT 1 FROM import_evidence r JOIN projects p ON p.id=NEW.project_id
    WHERE r.historical_task_id=NEW.id AND r.historical_wave_id IS NOT p.wave_id)
BEGIN SELECT RAISE(ABORT, 'Task would change historical input ancestry'); END;

CREATE TRIGGER task_first_agent_turn AFTER INSERT ON session_events
WHEN NEW.kind='started' AND NEW.task_id IS NOT NULL BEGIN
    UPDATE tasks SET started_at=NEW.observed_at WHERE id=NEW.task_id AND started_at IS NULL;
END;

CREATE TRIGGER retain_task_agent_turn BEFORE DELETE ON session_events
WHEN OLD.kind='started' AND OLD.task_id IS NOT NULL
    AND NOT EXISTS(SELECT 1 FROM import_evidence WHERE historical_task_id=OLD.task_id)
    AND NOT EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=OLD.task_id AND seq!=OLD.seq)
BEGIN SELECT RAISE(ABORT,'Cannot remove the last work of a started Task'); END;

CREATE TRIGGER validate_task_started_update BEFORE UPDATE OF started_at ON tasks BEGIN
    SELECT CASE WHEN OLD.started_at IS NOT NULL AND NEW.started_at IS NOT OLD.started_at
        THEN RAISE(ABORT,'Task first assignment time cannot change') END;
    SELECT CASE WHEN (NEW.started_at IS NOT NULL) != (
        EXISTS(SELECT 1 FROM import_evidence WHERE historical_task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM flow_sessions f JOIN flow_events e ON e.flow_id=f.id
            WHERE f.task_id=NEW.id AND e.kind='operation_started') OR
        EXISTS(SELECT 1 FROM task_events e WHERE e.task_id=NEW.id
            AND json_extract(e.kind_json,'$.kind')='started')
    ) THEN RAISE(ABORT,'Started requires recorded Task work') END;
END;
