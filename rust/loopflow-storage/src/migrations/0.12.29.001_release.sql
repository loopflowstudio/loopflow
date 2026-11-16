-- draft: project_status_chapters
-- Retain only the identity evidence needed for one-time provider adoption.
-- NULL: converted/new; 1: old current; 0: old noncurrent; -1: no receipt.
-- The marker is cleared after confirmed provider conversion. It owns no plan.
ALTER TABLE projects ADD COLUMN status TEXT NOT NULL DEFAULT 'started'
    CHECK (status IN ('backlog','planned','started','paused','completed','canceled'));
ALTER TABLE projects ADD COLUMN flow TEXT NOT NULL DEFAULT '';
ALTER TABLE projects ADD COLUMN legacy_current INTEGER
    CHECK (legacy_current IN (-1, 0, 1));
-- Planning-only Projects may have been observed before local Task allocation.
-- Preserve their exact provider identities so the first refresh can convert them.
INSERT INTO projects (id,wave_id,external_project_id,created_at,project_slug,
    project_name,project_prompt_context,pm_snapshot_synced_at,updated_at)
SELECT 'project_adoption_' || s.wave_id || '_' || json_extract(p.value,'$.id'),
    s.wave_id,json_extract(p.value,'$.id'),s.synced_at,
    json_extract(p.value,'$.slug'),json_extract(p.value,'$.name'),'',s.synced_at,s.synced_at
FROM pm_snapshots s, json_each(s.payload,'$.projects') p
WHERE NOT EXISTS (SELECT 1 FROM projects
    WHERE external_project_id=json_extract(p.value,'$.id'));
INSERT INTO projects (id,wave_id,external_project_id,created_at,project_slug,
    project_name,project_prompt_context,pm_snapshot_synced_at,updated_at)
SELECT 'project_adoption_' || c.wave_id || '_' || c.project_id,c.wave_id,c.project_id,
    COALESCE(json_extract(c.receipt,'$.created_at'),0),c.chapter_id,c.chapter_id,'',0,
    COALESCE(json_extract(c.receipt,'$.created_at'),0)
FROM wave_chapters c
WHERE NOT EXISTS (SELECT 1 FROM projects WHERE external_project_id=c.project_id);
UPDATE projects SET legacy_current = COALESCE((
    SELECT current FROM wave_chapters
    WHERE wave_chapters.wave_id = projects.wave_id
      AND wave_chapters.project_id = projects.external_project_id
), -1);
UPDATE projects SET flow = COALESCE((
    SELECT json_extract(p.value,'$.flows.recommended')
    FROM pm_snapshots s, json_each(s.payload,'$.projects') p
    WHERE s.wave_id=projects.wave_id
      AND json_extract(p.value,'$.id')=projects.external_project_id
), '');
-- Old cached snapshots must decode before their first network refresh, too.
UPDATE pm_snapshots SET payload = json_set(payload, '$.projects', json(COALESCE((
    SELECT json_group_array(json_remove(json_set(p.value,
        '$.flow',COALESCE(json_extract(p.value,'$.flows.recommended'),''),
        '$.status', CASE
            WHEN (SELECT current FROM wave_chapters c WHERE c.project_id=json_extract(p.value,'$.id'))=1 THEN 'started'
            WHEN EXISTS (SELECT 1 FROM wave_chapters c WHERE c.project_id=json_extract(p.value,'$.id')
                AND json_extract(c.receipt,'$.phase')='complete') THEN 'completed'
            WHEN EXISTS (SELECT 1 FROM wave_chapters c WHERE c.project_id=json_extract(p.value,'$.id')) THEN 'planned'
            WHEN json_array_length(pm_snapshots.payload,'$.projects')=1 THEN 'started'
            ELSE 'planned' END
    ), '$.flows'))
    FROM json_each(pm_snapshots.payload,'$.projects') p
), '[]')));
UPDATE projects SET status = COALESCE((
    SELECT json_extract(p.value,'$.status')
    FROM pm_snapshots s, json_each(s.payload,'$.projects') p
    WHERE s.wave_id=projects.wave_id AND json_extract(p.value,'$.id')=projects.external_project_id
), CASE legacy_current WHEN 1 THEN 'started' ELSE 'planned' END);
DROP TABLE wave_chapters;

-- draft: record_execs
CREATE TABLE execs (
    id TEXT PRIMARY KEY,
    trace_id TEXT NOT NULL,
    parent_exec_id TEXT,
    via_agent INTEGER CHECK (via_agent IN (0, 1)),
    caller_session_id TEXT,
    caller_provider_generation INTEGER,
    command TEXT,
    repo TEXT,
    cwd TEXT,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    outcome TEXT CHECK (outcome IN ('succeeded', 'failed', 'interrupted')),
    exit_code INTEGER,
    error TEXT,
    signal TEXT
);
CREATE INDEX execs_parent ON execs(parent_exec_id, started_at, id);
CREATE INDEX execs_trace ON execs(trace_id, started_at, id);
CREATE INDEX execs_recent ON execs(started_at DESC, id);

-- draft: session_ownership
ALTER TABLE tasks ADD COLUMN started_at INTEGER;

ALTER TABLE tasks ADD COLUMN current_invocation_id TEXT REFERENCES flow_sessions(id);

CREATE TABLE "flow_sessions" (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    task_id TEXT REFERENCES tasks(id) ON DELETE RESTRICT,
    invocation_json TEXT NOT NULL CHECK (json_valid(invocation_json)),
    step_index INTEGER NOT NULL CHECK (step_index >= 0),
    iteration INTEGER NOT NULL CHECK (iteration >= 0),
    position_version INTEGER NOT NULL CHECK (position_version > 0),
    worker_generation INTEGER NOT NULL CHECK (worker_generation >= 0),
    claim_json TEXT,
    failure_json TEXT,
    updated_at INTEGER NOT NULL,
    review_json TEXT,
    state TEXT NOT NULL CHECK (state IN ('current', 'completed', 'replaced')),
    ended_at INTEGER, pending_session_id TEXT REFERENCES "agent_sessions"(id), cwd TEXT, message TEXT, model TEXT, wave_id TEXT REFERENCES waves(id) ON DELETE RESTRICT, selected_start INTEGER REFERENCES session_events(seq), operation_start INTEGER REFERENCES flow_events(seq), current_capture INTEGER REFERENCES session_events(seq), unbound_repo TEXT,
    CHECK (json_type(invocation_json, '$.id') IS 'text'
        AND id = json_extract(invocation_json, '$.id')),
    CHECK ((state = 'current') = (ended_at IS NULL))
);

CREATE TABLE "flow_events" (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    flow_id TEXT NOT NULL REFERENCES flow_sessions(id),
    version INTEGER,
    node INTEGER NOT NULL,
    iterations TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('selected','consumed','operation_started','operation_completed')),
    session_event INTEGER REFERENCES session_events(seq),
    observed_at INTEGER,
    exec_id TEXT REFERENCES execs(id),
    operation_start INTEGER REFERENCES "flow_events"(seq),
    outcome TEXT,
    payload TEXT,
    UNIQUE(flow_id,kind,session_event),
    UNIQUE(operation_start),
    CHECK((kind IN ('selected','consumed')) = (session_event IS NOT NULL)),
    CHECK((kind='operation_completed') = (operation_start IS NOT NULL AND outcome IS NOT NULL))
);

CREATE TABLE "session_events" (
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

CREATE TABLE "agent_sessions" (
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

INSERT INTO flow_sessions(id,task_id,wave_id,invocation_json,step_index,iteration,
 position_version,worker_generation,claim_json,failure_json,updated_at,review_json,state)
SELECT json_extract(f.invocation_json,'$.id'),f.task_id,p.wave_id,f.invocation_json,
 f.step_index,f.iteration,f.position_version,f.worker_generation,f.claim_json,f.failure_json,
 f.updated_at,f.review_json,'current'
FROM task_flow_positions f JOIN tasks t ON t.id=f.task_id JOIN projects p ON p.id=t.project_id;
UPDATE tasks SET current_invocation_id=(SELECT id FROM flow_sessions WHERE task_id=tasks.id);
UPDATE tasks SET started_at=(SELECT MIN(created_at) FROM task_events WHERE task_id=tasks.id
 AND json_extract(kind_json,'$.kind')='started');
-- Rewrite only the saved graphs that still drive Tasks. This is a data
-- conversion at the released frontier, not an alternate runtime decoder.
WITH RECURSIVE edits AS (
 SELECT f.id,j.fullkey,j.path,j.key,
 row_number() OVER(PARTITION BY f.id ORDER BY length(j.fullkey) DESC,j.fullkey) AS n
 FROM flow_sessions f,json_tree(f.invocation_json) j
 WHERE j.type='object' AND j.key IN ('Skill','Op','Xor')
), rewritten(id,n,doc) AS (
 SELECT id,0,invocation_json FROM flow_sessions
 UNION ALL
 SELECT r.id,e.n,
 CASE e.key
 WHEN 'Skill' THEN json_set(r.doc,e.fullkey,json_remove(json_set(json_extract(r.doc,e.fullkey),
   '$.id',json_extract(r.doc,e.fullkey||'.policy.id'),
   '$.human',json(CASE WHEN json_extract(r.doc,e.fullkey||'.policy.human')=1 THEN 'true' ELSE 'false' END),
   '$.repeat',json_extract(r.doc,e.fullkey||'.policy.repeat'),
   '$.sources',json(COALESCE(json_extract(r.doc,e.fullkey||'.flow_parents'),'[]'))),
   '$.policy','$.flow_parents'))
 WHEN 'Op' THEN json_remove(json_set(r.doc,e.path||'.Command',
   json_remove(json_set(json_extract(r.doc,e.fullkey),'$.sources',
     json(COALESCE(json_extract(r.doc,e.fullkey||'.flow_parents'),'[]'))),'$.flow_parents')),e.fullkey)
 ELSE json_set(r.doc,e.fullkey,json_remove(json_set(json_extract(r.doc,e.fullkey),'$.sources',
   json(COALESCE(json_extract(r.doc,e.fullkey||'.flow_parents'),'[]'))),'$.flow_parents')) END
 FROM rewritten r JOIN edits e ON e.id=r.id AND e.n=r.n+1
)
UPDATE flow_sessions SET invocation_json=(SELECT doc FROM rewritten WHERE id=flow_sessions.id ORDER BY n DESC LIMIT 1);

-- Retain the selected conversation and feedback, without importing its past turns.
INSERT INTO agent_sessions(id,title,title_source,ready_summary,created_at,kind,interactive,
 task_id,wave_id,flow_session_id,input_published,cwd,skill,work_source)
SELECT CASE WHEN f.human=1 THEN f.task_id||':'||s.id||':'||f.flow||':'||f.node_id||':'||f.iteration
 ELSE f.session_run_id END,COALESCE(t.issue_title,t.issue_identifier),'generated',f.ready_summary,
 f.updated_at,CASE WHEN f.human=1 THEN 'flow_review' ELSE 'conversation' END,f.human,
 f.task_id,s.wave_id,s.id,f.session_run_id IS NOT NULL,COALESCE(t.worktree,''),f.step,'declared'
FROM task_flow_positions f JOIN tasks t ON t.id=f.task_id JOIN flow_sessions s ON s.task_id=f.task_id
WHERE f.session_run_id IS NOT NULL OR f.human=1;
INSERT INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload)
SELECT a.id,'captured',COALESCE(f.session_run_id,'run_'||lower(hex(randomblob(16)))),
 a.task_id,a.wave_id,a.created_at,json_object('cwd',a.cwd,'skill',a.skill,
 'flow_session_id',a.flow_session_id,'work_source','declared')
FROM agent_sessions a JOIN task_flow_positions f ON f.task_id=a.task_id;
UPDATE agent_sessions SET current_capture=(SELECT seq FROM session_events WHERE session_id=agent_sessions.id);
UPDATE flow_sessions SET current_capture=(SELECT current_capture FROM agent_sessions WHERE flow_session_id=flow_sessions.id),
 pending_session_id=(SELECT id FROM agent_sessions WHERE flow_session_id=flow_sessions.id AND kind='flow_review');

DROP TRIGGER IF EXISTS task_chapter_started;
DROP TABLE task_flow_positions;
DROP TABLE observation_outbox;
DROP TABLE provider_deliveries;
DROP TABLE run_events;

CREATE INDEX task_invocation_history ON "flow_sessions"(task_id, updated_at, id);

CREATE TRIGGER validate_task_started_insert BEFORE INSERT ON tasks
WHEN NEW.started_at IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'Started requires a Task Run'); END;

CREATE TRIGGER validate_task_invocation BEFORE UPDATE OF current_invocation_id ON tasks
WHEN NEW.current_invocation_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM "flow_sessions" WHERE id=NEW.current_invocation_id AND task_id=NEW.id
)
BEGIN SELECT RAISE(ABORT, 'Task invocation names another Task'); END;

CREATE TRIGGER validate_invocation_wave_insert BEFORE INSERT ON "flow_sessions"
WHEN NEW.task_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
    WHERE t.id=NEW.task_id AND p.wave_id=NEW.wave_id
)
BEGIN SELECT RAISE(ABORT, 'Invocation Task and Wave disagree'); END;

CREATE TRIGGER validate_invocation_wave_update BEFORE UPDATE OF task_id,wave_id ON "flow_sessions"
WHEN NEW.task_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
    WHERE t.id=NEW.task_id AND p.wave_id=NEW.wave_id
)
BEGIN SELECT RAISE(ABORT, 'Invocation Task and Wave disagree'); END;

CREATE TRIGGER validate_invocation_cwd_insert BEFORE INSERT ON "flow_sessions"
WHEN NEW.task_id IS NOT NULL AND NEW.cwd IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'A Task invocation runs in the Task worktree'); END;

CREATE TRIGGER validate_invocation_cwd_update BEFORE UPDATE OF task_id,cwd ON "flow_sessions"
WHEN NEW.task_id IS NOT NULL AND NEW.cwd IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'A Task invocation runs in the Task worktree'); END;

CREATE INDEX flow_events_history ON flow_events(flow_id,seq);

CREATE INDEX flow_pending_review ON flow_sessions(pending_session_id, state);

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
    session_id, captured_event, CASE WHEN json_valid(payload) THEN CASE WHEN json_extract(payload,'$.source')='manifest.json' AND json_extract(payload,'$.evidence.schema_version')=1 AND json_extract(payload,'$.evidence.artifact_key')=json_extract(payload,'$.input_id') AND receipt_key=json_extract(payload,'$.input_id')||':manifest.json' THEN json_extract(payload,'$.evidence.flow.kind') END END)
    WHERE kind='observed' AND substr(receipt_key,-14)=':manifest.json';

CREATE UNIQUE INDEX session_capture_key ON session_events(receipt_key) WHERE kind='captured';

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

CREATE TRIGGER task_first_agent_turn AFTER INSERT ON session_events
WHEN NEW.kind='started' AND NEW.task_id IS NOT NULL BEGIN
    UPDATE tasks SET started_at=NEW.observed_at WHERE id=NEW.task_id AND started_at IS NULL;
END;

CREATE TRIGGER retain_task_agent_turn BEFORE DELETE ON session_events
WHEN OLD.kind='started' AND OLD.task_id IS NOT NULL
    AND NOT EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=OLD.task_id AND seq!=OLD.seq)
BEGIN SELECT RAISE(ABORT,'Cannot remove the last work of a started Task'); END;

CREATE TRIGGER validate_task_started_update BEFORE UPDATE OF started_at ON tasks BEGIN
    SELECT CASE WHEN OLD.started_at IS NOT NULL AND NEW.started_at IS NOT OLD.started_at
        THEN RAISE(ABORT,'Task first assignment time cannot change') END;
    SELECT CASE WHEN (NEW.started_at IS NOT NULL) != (
        EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM flow_sessions f JOIN flow_events e ON e.flow_id=f.id
            WHERE f.task_id=NEW.id AND e.kind='operation_started') OR
        EXISTS(SELECT 1 FROM task_events e WHERE e.task_id=NEW.id
            AND json_extract(e.kind_json,'$.kind')='started')
    ) THEN RAISE(ABORT,'Started requires recorded Task work') END;
END;

CREATE TRIGGER validate_flow_repository_insert BEFORE INSERT ON flow_sessions
WHEN NEW.unbound_repo IS NOT NULL AND NEW.wave_id IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'Bound Flow repository belongs to its Wave'); END;

CREATE TRIGGER validate_flow_repository_update BEFORE UPDATE OF wave_id,unbound_repo ON flow_sessions
WHEN NEW.unbound_repo IS NOT NULL AND NEW.wave_id IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'Bound Flow repository belongs to its Wave'); END;

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
