-- name: own_conversation_ancestry
-- id: 7a5cbe8208b34af886d520986a378458
-- depends_on: name_conversation_and_flow_owners

ALTER TABLE agent_sessions ADD COLUMN task_id TEXT REFERENCES tasks(id);
ALTER TABLE agent_sessions ADD COLUMN wave_id TEXT REFERENCES waves(id);
ALTER TABLE agent_sessions ADD COLUMN flow_session_id TEXT REFERENCES flow_sessions(id);
ALTER TABLE agent_sessions ADD COLUMN work_source TEXT
    CHECK(work_source IN ('declared','checkout','inherited','bound'));
ALTER TABLE agent_sessions ADD COLUMN bound_at INTEGER;
UPDATE agent_sessions SET (task_id,wave_id,flow_session_id,work_source)=(
    SELECT task_id,wave_id,invocation_id,work_source FROM runs WHERE id=current_run_id
);
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

-- First assignment includes binding an existing conversation. That ownership
-- starts the Task without rewriting earlier work or usage attribution.
DROP TRIGGER validate_task_started_update;
CREATE TRIGGER validate_task_started_update BEFORE UPDATE OF started_at ON tasks BEGIN
    SELECT CASE WHEN OLD.started_at IS NOT NULL AND NEW.started_at IS NOT OLD.started_at
        THEN RAISE(ABORT,'Task first assignment time cannot change') END;
    SELECT CASE WHEN (NEW.started_at IS NOT NULL) != (
        EXISTS(SELECT 1 FROM runs WHERE task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=NEW.id)
    ) THEN RAISE(ABORT,'Started requires recorded Task work') END;
END;
CREATE TRIGGER task_first_agent_turn AFTER INSERT ON session_events
WHEN NEW.kind='started' AND NEW.task_id IS NOT NULL BEGIN
    UPDATE tasks SET started_at=NEW.observed_at WHERE id=NEW.task_id AND started_at IS NULL;
END;
CREATE TRIGGER retain_task_agent_turn BEFORE DELETE ON session_events
WHEN OLD.kind='started' AND OLD.task_id IS NOT NULL
    AND NOT EXISTS(SELECT 1 FROM runs WHERE task_id=OLD.task_id)
    AND NOT EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=OLD.task_id AND seq!=OLD.seq)
BEGIN SELECT RAISE(ABORT,'Cannot remove the last work of a started Task'); END;

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
