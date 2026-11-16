-- Task observations depend on workflow facts, not transcript/activity traffic.
-- Keep the existing Work domains and their counters unchanged.
CREATE TEMP TABLE task_update_prior_revisions AS SELECT * FROM store_revisions;
DROP TABLE store_revisions;
CREATE TABLE store_revisions (
    domain TEXT PRIMARY KEY NOT NULL CHECK (domain IN ('planning','sessions','graphs','processes','usage','task_updates')),
    revision INTEGER NOT NULL
) WITHOUT ROWID;
INSERT INTO store_revisions SELECT * FROM task_update_prior_revisions;
DROP TABLE task_update_prior_revisions;
INSERT INTO store_revisions VALUES('task_updates',0);

CREATE TRIGGER task_update_task_graphs_insert AFTER INSERT ON task_graphs
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_task_graphs_update AFTER UPDATE ON task_graphs
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_task_graphs_delete AFTER DELETE ON task_graphs
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_graph_processes_insert AFTER INSERT ON graph_processes
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_graph_processes_update AFTER UPDATE ON graph_processes
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_graph_processes_delete AFTER DELETE ON graph_processes
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_graph_process_steps_insert AFTER INSERT ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_graph_process_steps_update AFTER UPDATE ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_graph_process_steps_delete AFTER DELETE ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_task_comments_insert AFTER INSERT ON task_comments
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_task_comments_update AFTER UPDATE ON task_comments
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_task_comments_delete AFTER DELETE ON task_comments
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_relocated_task_processes_insert AFTER INSERT ON relocated_task_processes
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_relocated_task_processes_update AFTER UPDATE ON relocated_task_processes
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_relocated_task_processes_delete AFTER DELETE ON relocated_task_processes
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_tasks_insert AFTER INSERT ON tasks
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_tasks_delete AFTER DELETE ON tasks
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_tasks_update AFTER UPDATE OF issue_identifier,worktree,checkout_machine_id ON tasks
WHEN NEW.issue_identifier IS NOT OLD.issue_identifier OR NEW.worktree IS NOT OLD.worktree OR NEW.checkout_machine_id IS NOT OLD.checkout_machine_id
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_agent_sessions_insert AFTER INSERT ON agent_sessions
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_agent_sessions_delete AFTER DELETE ON agent_sessions
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_agent_sessions_update AFTER UPDATE OF task_id,primary_scope,cwd,interactive,completed_at ON agent_sessions
WHEN NEW.task_id IS NOT OLD.task_id OR NEW.primary_scope IS NOT OLD.primary_scope OR NEW.cwd IS NOT OLD.cwd OR NEW.interactive IS NOT OLD.interactive OR NEW.completed_at IS NOT OLD.completed_at
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_machines_insert AFTER INSERT ON machines
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_machines_delete AFTER DELETE ON machines
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_machines_update AFTER UPDATE OF is_local ON machines
WHEN NEW.is_local IS NOT OLD.is_local
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_processes_insert AFTER INSERT ON processes
WHEN (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.id)
    OR EXISTS(SELECT 1 FROM graph_process_steps WHERE lf_process_id=NEW.id)
    OR EXISTS(SELECT 1 FROM task_graphs WHERE lf_process_id=NEW.id)
    OR (NEW.kind='agent' AND EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.attached_lf_process_id)))
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_processes_update AFTER UPDATE ON processes
WHEN (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.id)
    OR EXISTS(SELECT 1 FROM graph_process_steps WHERE lf_process_id=OLD.id)
    OR EXISTS(SELECT 1 FROM task_graphs WHERE lf_process_id=OLD.id)
    OR (OLD.kind='agent' AND EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.attached_lf_process_id))) OR (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.id)
    OR EXISTS(SELECT 1 FROM graph_process_steps WHERE lf_process_id=NEW.id)
    OR EXISTS(SELECT 1 FROM task_graphs WHERE lf_process_id=NEW.id)
    OR (NEW.kind='agent' AND EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.attached_lf_process_id)))
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_processes_delete AFTER DELETE ON processes
WHEN (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.id)
    OR EXISTS(SELECT 1 FROM graph_process_steps WHERE lf_process_id=OLD.id)
    OR EXISTS(SELECT 1 FROM task_graphs WHERE lf_process_id=OLD.id)
    OR (OLD.kind='agent' AND EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.attached_lf_process_id)))
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_session_events_insert AFTER INSERT ON session_events
WHEN (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.lf_process_id)
    OR (CASE WHEN NEW.kind='observed' AND json_valid(NEW.payload)
        THEN json_extract(NEW.payload,'$.source')='task_update'
         AND json_extract(NEW.payload,'$.state') IN ('baseline','dispatched') END))
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_session_events_update AFTER UPDATE ON session_events
WHEN (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.lf_process_id)
    OR (CASE WHEN OLD.kind='observed' AND json_valid(OLD.payload)
        THEN json_extract(OLD.payload,'$.source')='task_update'
         AND json_extract(OLD.payload,'$.state') IN ('baseline','dispatched') END)) OR (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.lf_process_id)
    OR (CASE WHEN NEW.kind='observed' AND json_valid(NEW.payload)
        THEN json_extract(NEW.payload,'$.source')='task_update'
         AND json_extract(NEW.payload,'$.state') IN ('baseline','dispatched') END))
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_session_events_delete AFTER DELETE ON session_events
WHEN (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.lf_process_id)
    OR (CASE WHEN OLD.kind='observed' AND json_valid(OLD.payload)
        THEN json_extract(OLD.payload,'$.source')='task_update'
         AND json_extract(OLD.payload,'$.state') IN ('baseline','dispatched') END))
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
