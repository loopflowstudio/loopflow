-- name: agent_session_admission
-- id: a89e13f5c5d2439da777324d31101db8
-- depends_on: record_flow_turn_caller, retain_conversation_engine, flow_operation_history

-- Keep captured input and its publication on the conversation. Historical Run
-- rows remain import evidence until their complete preservation is verified.
DROP TRIGGER validate_conversation_ancestry_insert;
DROP TRIGGER validate_conversation_ancestry_update;
DROP TRIGGER validate_flow_conversation_parents;
DROP TRIGGER validate_task_conversation_parents;
DROP TRIGGER validate_project_conversation_parents;
DROP TRIGGER task_first_conversation_insert;
DROP TRIGGER task_first_conversation_bind;
DROP TRIGGER validate_task_started_update;
CREATE TABLE agent_session_admission (
    id TEXT PRIMARY KEY NOT NULL,
    input_id TEXT NOT NULL UNIQUE,
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
INSERT INTO agent_session_admission(id,input_id,title,title_source,ready_summary,completed_at,created_at,request,driver_exec_id,driver_generation,provider_generation,provider_exec_id,interactive,kind,repo,provider_endpoint,provider_thread,provider_pid,provider_started_at,task_id,wave_id,flow_session_id,work_source,bound_at,input_published,cwd,skill,provider,model,node,iterations)
SELECT s.id,s.current_run_id,s.title,s.title_source,s.ready_summary,s.completed_at,s.created_at,s.request,s.driver_exec_id,s.driver_generation,s.provider_generation,s.provider_exec_id,s.interactive,s.kind,s.repo,s.provider_endpoint,s.provider_thread,s.provider_pid,s.provider_started_at,s.task_id,s.wave_id,s.flow_session_id,s.work_source,s.bound_at,r.published,r.cwd,r.skill,r.provider,r.model,r.node,r.iterations
FROM agent_sessions s JOIN runs r ON r.id=s.current_run_id AND r.session_id=s.id;
DROP TABLE agent_sessions;
ALTER TABLE agent_session_admission RENAME TO agent_sessions;
CREATE INDEX open_sessions ON "agent_sessions"(created_at, id) WHERE completed_at IS NULL;
CREATE INDEX session_inventory ON "agent_sessions"(interactive, completed_at, title, id);
CREATE INDEX session_repo_inventory ON "agent_sessions"(repo, interactive, completed_at, title, id);
CREATE INDEX conversation_task_inventory ON agent_sessions(task_id,title,id);
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
CREATE TRIGGER validate_task_started_update BEFORE UPDATE OF started_at ON tasks BEGIN
    SELECT CASE WHEN OLD.started_at IS NOT NULL AND NEW.started_at IS NOT OLD.started_at
        THEN RAISE(ABORT,'Task first assignment time cannot change') END;
    SELECT CASE WHEN (NEW.started_at IS NOT NULL) != (
        EXISTS(SELECT 1 FROM runs WHERE task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM flow_sessions f JOIN flow_events e ON e.flow_id=f.id
            WHERE f.task_id=NEW.id AND e.kind='operation_started')
    ) THEN RAISE(ABORT,'Started requires recorded Task work') END;
END;

DROP TRIGGER validate_invocation_attempt;
DROP TRIGGER validate_initial_invocation_attempt;
CREATE TEMP TABLE flow_input_refs AS SELECT id,current_run_id FROM flow_sessions;
ALTER TABLE flow_sessions DROP COLUMN current_run_id;
ALTER TABLE flow_sessions ADD COLUMN current_run_id TEXT;
UPDATE flow_sessions SET current_run_id=(SELECT current_run_id FROM flow_input_refs WHERE id=flow_sessions.id);
DROP TABLE flow_input_refs;

CREATE TRIGGER validate_flow_input BEFORE UPDATE OF current_run_id ON flow_sessions
WHEN NEW.current_run_id IS NOT NULL AND NOT EXISTS(
    SELECT 1 FROM agent_sessions WHERE input_id=NEW.current_run_id AND flow_session_id=NEW.id
) BEGIN SELECT RAISE(ABORT,'Flow input belongs to another conversation'); END;

-- These are immutable artifact references in conversation history, with no
-- execution state, result, retry ordinal or independent lifecycle.
CREATE TABLE agent_session_inputs (
    input_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES agent_sessions(id),
    caller_input_id TEXT
);
INSERT INTO agent_session_inputs(input_id,session_id,caller_input_id)
SELECT id,session_id,caller_run_id FROM runs WHERE session_id IS NOT NULL;
CREATE INDEX agent_session_input_history ON agent_session_inputs(session_id,input_id);
