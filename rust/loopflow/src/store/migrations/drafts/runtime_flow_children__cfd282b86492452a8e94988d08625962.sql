-- name: runtime_flow_children
-- id: cfd282b86492452a8e94988d08625962
-- depends_on: index_session_process_owners

ALTER TABLE flow_sessions ADD COLUMN parent_id TEXT REFERENCES flow_sessions(id);
CREATE UNIQUE INDEX flow_current_child ON flow_sessions(parent_id)
    WHERE parent_id IS NOT NULL AND state='current';
CREATE INDEX flow_parent ON flow_sessions(parent_id);

-- Membership is a projection of the one Task pointer and runtime parentage.
CREATE VIEW managed_flows AS WITH RECURSIVE managed(task_id,flow_id) AS (
    SELECT id,current_invocation_id FROM tasks WHERE current_invocation_id IS NOT NULL
    UNION ALL SELECT m.task_id,f.id FROM flow_sessions f JOIN managed m ON f.parent_id=m.flow_id
) SELECT task_id,flow_id FROM managed;

CREATE TRIGGER validate_flow_parent BEFORE INSERT ON flow_sessions
WHEN NEW.parent_id IS NOT NULL BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM flow_sessions p WHERE p.id=NEW.parent_id
        AND p.state='current' AND p.task_id IS NEW.task_id AND p.wave_id IS NEW.wave_id)
        THEN RAISE(ABORT,'runtime child must retain its current parent and Work') END;
END;

CREATE TRIGGER retain_flow_parent BEFORE UPDATE OF parent_id,task_id,wave_id ON flow_sessions
WHEN NEW.parent_id IS NOT OLD.parent_id OR ((NEW.parent_id IS NOT NULL OR
    EXISTS(SELECT 1 FROM flow_sessions child WHERE child.parent_id=NEW.id)) AND
    (NEW.task_id IS NOT OLD.task_id OR NEW.wave_id IS NOT OLD.wave_id)) BEGIN
    SELECT RAISE(ABORT,'runtime child ancestry is immutable');
END;
