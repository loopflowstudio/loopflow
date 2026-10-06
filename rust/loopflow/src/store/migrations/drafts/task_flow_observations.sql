
-- CI repair retains its existing per-Task hold. Saved Flow scheduling is retired.
ALTER TABLE tasks DROP COLUMN automation_exec_id;
ALTER TABLE tasks DROP COLUMN automation_retry_key;
ALTER TABLE tasks DROP COLUMN automation_retries;
ALTER TABLE tasks DROP COLUMN automation_checked_at;
ALTER TABLE tasks DROP COLUMN automation_detail;

-- Feedback remains Session history; it no longer grants readiness or settlement.
INSERT INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload,captured_event)
SELECT id,'observed','legacy_review_feedback',task_id,wave_id,unixepoch(),
    json_object('type','legacy_review_feedback','summary',ready_summary),current_capture
FROM agent_sessions WHERE ready_summary IS NOT NULL;
ALTER TABLE agent_sessions DROP COLUMN ready_summary;

-- Keep unresolved review boundaries as observations on their original conversation.
-- Neither conversion nor later native resume approves or advances the saved Flow.
INSERT INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload,captured_event)
SELECT s.id,'observed','legacy_flow_review',s.task_id,s.wave_id,unixepoch(),
    json_object('type','legacy_flow_review','flow_id',s.flow_session_id,
        'pending',s.completed_at IS NULL AND EXISTS(SELECT 1 FROM flow_sessions f WHERE f.pending_session_id=s.id AND f.state='current'),
        'completed_at',s.completed_at),s.current_capture
FROM agent_sessions s WHERE s.kind='flow_review';
UPDATE agent_sessions SET kind='conversation' WHERE kind='flow_review';

-- Each saved Flow's name, state and last position stay as an observation on
-- every conversation it opened. None of it is resumable state.
DROP TRIGGER validate_task_invocation;
ALTER TABLE tasks DROP COLUMN current_invocation_id;
INSERT INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload,captured_event)
SELECT s.id,'observed','legacy_flow:'||f.id,s.task_id,s.wave_id,unixepoch(),
    json_object('type','legacy_flow','flow_id',f.id,
        'flow',CASE WHEN json_valid(f.invocation_json) THEN json_extract(f.invocation_json,'$.flow') END,
        'state',f.state,'ended_at',f.ended_at,'step_index',f.step_index,'iteration',f.iteration,
        'cursor',CASE WHEN json_valid(f.review_json) THEN json(f.review_json) END,
        'failure',CASE WHEN json_valid(f.failure_json) THEN json(f.failure_json) END,
        'node',s.node,'iterations',CASE WHEN json_valid(s.iterations) THEN json(s.iterations) END),
    s.current_capture
FROM agent_sessions s JOIN flow_sessions f ON f.id=s.flow_session_id;
-- A Task whose only start evidence was its Flow row keeps that evidence.
DROP TRIGGER validate_task_started_update;
UPDATE tasks SET started_at=(SELECT MIN(updated_at) FROM flow_sessions WHERE task_id=tasks.id)
WHERE started_at IS NULL AND EXISTS(SELECT 1 FROM flow_sessions WHERE task_id=tasks.id);

DROP TRIGGER validate_conversation_ancestry_insert;
DROP TRIGGER validate_conversation_ancestry_update;
DROP TRIGGER validate_flow_conversation_parents;
DROP TRIGGER validate_flow_capture;
ALTER TABLE agent_sessions DROP COLUMN flow_session_id;
-- A conversation's Flow position is its step row, written by the driver.
ALTER TABLE agent_sessions DROP COLUMN node;
ALTER TABLE agent_sessions DROP COLUMN iterations;
UPDATE flow_sessions SET operation_start=NULL;
DROP TABLE flow_events;
DROP TABLE flow_sessions;

CREATE TRIGGER validate_conversation_ancestry_insert BEFORE INSERT ON agent_sessions BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id=NEW.wave_id
    ) THEN RAISE(ABORT,'AgentSession Task and Wave disagree') END;
END;

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

-- One row per Flow run, written once by its driver: the Flow's name and its
-- graph as compiled at launch. Running, finished and failed are the driver
-- Exec's; nothing here is updated.
CREATE TABLE flow_execs (
    exec_id TEXT PRIMARY KEY REFERENCES execs(id),
    flow TEXT NOT NULL,
    graph TEXT NOT NULL CHECK (json_valid(graph))
) STRICT;

-- One row per step the driver started, in launch order. The step's result is
-- its own Exec's exit.
CREATE TABLE flow_exec_steps (
    seq INTEGER PRIMARY KEY,
    flow_exec_id TEXT NOT NULL REFERENCES flow_execs(exec_id),
    exec_id TEXT NOT NULL UNIQUE REFERENCES execs(id),
    node INTEGER NOT NULL,
    iterations TEXT NOT NULL CHECK (json_valid(iterations))
) STRICT;
CREATE INDEX flow_exec_steps_flow ON flow_exec_steps(flow_exec_id, seq);

CREATE TRIGGER flow_execs_are_append_only BEFORE UPDATE ON flow_execs BEGIN
    SELECT RAISE(ABORT,'A Flow record is append-only');
END;
CREATE TRIGGER flow_exec_steps_are_append_only BEFORE UPDATE ON flow_exec_steps BEGIN
    SELECT RAISE(ABORT,'A Flow record is append-only');
END;
CREATE TRIGGER validate_flow_exec_step BEFORE INSERT ON flow_exec_steps BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM execs step
        WHERE step.id=NEW.exec_id AND step.parent_exec_id=NEW.flow_exec_id
    ) THEN RAISE(ABORT,'A Flow step is an Exec its driver started') END;
END;

-- Started is set once, by recorded work: a conversation, a native turn, a Flow
-- run from the Task's checkout, or an imported start.
CREATE TRIGGER validate_task_started_update BEFORE UPDATE OF started_at ON tasks BEGIN
    SELECT CASE WHEN OLD.started_at IS NOT NULL AND NEW.started_at IS NOT OLD.started_at
        THEN RAISE(ABORT,'Task first assignment time cannot change') END;
    SELECT CASE WHEN NEW.started_at IS NOT NULL AND NOT (
        EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM flow_execs f JOIN execs driver ON driver.id=f.exec_id WHERE NEW.worktree!=''
            AND (driver.cwd=rtrim(NEW.worktree,'/') OR instr(driver.cwd,rtrim(NEW.worktree,'/')||'/')=1)) OR
        EXISTS(SELECT 1 FROM task_events e WHERE e.task_id=NEW.id
            AND json_extract(e.kind_json,'$.kind')='started')
    ) THEN RAISE(ABORT,'Started requires recorded Task work') END;
END;

-- A Task's workflow: its name and graph as captured when the Task took it up.
-- The newest row is the Task's; replacing a workflow appends another.
CREATE TABLE task_workflows (
    id INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    workflow TEXT NOT NULL,
    graph TEXT NOT NULL CHECK (json_valid(graph)),
    started_at INTEGER NOT NULL
) STRICT;
CREATE INDEX task_workflows_task ON task_workflows(task_id, id);

-- One row per edge `lf task run` set out on, and the Exec that ran it: the
-- driver of that edge's Flow. Position is read from these and their Execs.
CREATE TABLE task_workflow_traversals (
    seq INTEGER PRIMARY KEY,
    task_workflow_id INTEGER NOT NULL REFERENCES task_workflows(id) ON DELETE CASCADE,
    edge INTEGER NOT NULL,
    exec_id TEXT NOT NULL UNIQUE REFERENCES execs(id)
) STRICT;
CREATE INDEX task_workflow_traversals_workflow ON task_workflow_traversals(task_workflow_id, seq);

CREATE TRIGGER task_workflows_are_append_only BEFORE UPDATE ON task_workflows BEGIN
    SELECT RAISE(ABORT,'A Task workflow record is append-only');
END;
CREATE TRIGGER task_workflow_traversals_are_append_only BEFORE UPDATE ON task_workflow_traversals BEGIN
    SELECT RAISE(ABORT,'A Task workflow record is append-only');
END;
