-- Execution owners consume close requests; Task cleanup decides eligibility.
ALTER TABLE processes ADD COLUMN close_requested TEXT
    CHECK(close_requested IN ('completed','abandoned','shell_pending','shell','retired_shell'));
ALTER TABLE processes ADD COLUMN close_task_id TEXT REFERENCES tasks(id);
CREATE INDEX processes_close_task ON processes(close_task_id) WHERE close_task_id IS NOT NULL;

-- Released shell receipts remain readable until their staged column removal.
-- Automatic closes are re-evaluated by cleanup; explicit Close survives upgrade.
UPDATE processes SET close_requested='shell_pending'
WHERE id IN (SELECT owner FROM desktop_shells WHERE close_requested=1);
UPDATE desktop_shells SET jobs_settled=1 WHERE owner IS NULL AND close_requested!=0;

CREATE TRIGGER cancel_task_owner_closes
AFTER UPDATE OF planning_completed,planning_completed_at,abandoned_at,worktree,branch ON tasks
WHEN NEW.planning_completed IS NOT OLD.planning_completed
  OR NEW.planning_completed_at IS NOT OLD.planning_completed_at
  OR NEW.abandoned_at IS NOT OLD.abandoned_at
  OR NEW.worktree IS NOT OLD.worktree OR NEW.branch IS NOT OLD.branch
BEGIN
    UPDATE processes SET close_requested=NULL,close_task_id=NULL WHERE close_task_id=NEW.id;
END;
CREATE TRIGGER cancel_session_owner_closes
AFTER UPDATE OF task_id,cwd,primary_scope,interactive,completed_at,transport_lf_process_id ON agent_sessions
WHEN NEW.task_id IS NOT OLD.task_id OR NEW.cwd IS NOT OLD.cwd
  OR NEW.primary_scope IS NOT OLD.primary_scope OR NEW.interactive IS NOT OLD.interactive
  OR NEW.completed_at IS NOT OLD.completed_at
  OR NEW.transport_lf_process_id IS NOT OLD.transport_lf_process_id
BEGIN
    UPDATE processes SET close_requested=NULL,close_task_id=NULL
    WHERE close_task_id IS NOT NULL AND id IN (OLD.transport_lf_process_id,NEW.transport_lf_process_id);
END;

-- Work starting after cleanup's decision revokes automatic completion requests.
CREATE TRIGGER cancel_graph_owner_closes_insert AFTER INSERT ON task_graphs BEGIN
    UPDATE processes SET close_requested=NULL,close_task_id=NULL
    WHERE close_task_id=NEW.task_id AND close_requested IN ('completed','retired_shell');
END;
CREATE TRIGGER cancel_graph_owner_closes_update AFTER UPDATE ON task_graphs
WHEN NEW.lf_process_id IS NOT OLD.lf_process_id OR NEW.run_start IS NOT OLD.run_start
BEGIN
    UPDATE processes SET close_requested=NULL,close_task_id=NULL
    WHERE close_task_id=NEW.task_id AND close_requested IN ('completed','retired_shell');
END;

CREATE TRIGGER cancel_active_owner_closes_insert AFTER INSERT ON session_activity
WHEN NEW.open_tools>0 OR NEW.pending_input>0 OR NEW.yielded=0
BEGIN
    UPDATE processes SET close_requested='shell_pending' WHERE close_requested='shell'
        AND id=(SELECT transport_lf_process_id FROM agent_sessions WHERE id=NEW.session_id);
    UPDATE processes SET close_requested=NULL,close_task_id=NULL
    WHERE close_requested IN ('completed','retired_shell') AND close_task_id IN (
        SELECT tw.id FROM tasks tw JOIN agent_sessions s ON s.id=NEW.session_id
        WHERE s.primary_scope IS NULL AND (s.task_id=tw.id OR (
            tw.checkout_machine_id=(SELECT id FROM machines WHERE is_local=1)
            AND tw.worktree!='' AND (s.cwd=rtrim(tw.worktree,'/')
                OR instr(s.cwd,rtrim(tw.worktree,'/')||'/')=1))));
END;

CREATE TRIGGER cancel_active_owner_closes_update AFTER UPDATE ON session_activity
WHEN NEW.open_tools>0 OR NEW.pending_input>0 OR NEW.yielded=0
BEGIN
    UPDATE processes SET close_requested='shell_pending' WHERE close_requested='shell'
        AND id=(SELECT transport_lf_process_id FROM agent_sessions WHERE id=NEW.session_id);
    UPDATE processes SET close_requested=NULL,close_task_id=NULL
    WHERE close_requested IN ('completed','retired_shell') AND close_task_id IN (
        SELECT tw.id FROM tasks tw JOIN agent_sessions s ON s.id=NEW.session_id
        WHERE s.primary_scope IS NULL AND (s.task_id=tw.id OR (
            tw.checkout_machine_id=(SELECT id FROM machines WHERE is_local=1)
            AND tw.worktree!='' AND (s.cwd=rtrim(tw.worktree,'/')
                OR instr(s.cwd,rtrim(tw.worktree,'/')||'/')=1))));
END;

CREATE TRIGGER cancel_nested_graph_owner_closes AFTER INSERT ON graph_processes BEGIN
    UPDATE processes SET close_requested=CASE WHEN close_requested='shell' THEN 'shell_pending' END,
        close_task_id=NULL
    WHERE close_requested IN ('completed','retired_shell','shell') AND id IN (
        WITH RECURSIVE ancestors(id) AS (
            SELECT parent_lf_process_id FROM processes WHERE id=NEW.lf_process_id
            UNION SELECT p.parent_lf_process_id FROM processes p JOIN ancestors a ON p.id=a.id)
        SELECT id FROM ancestors);
END;
