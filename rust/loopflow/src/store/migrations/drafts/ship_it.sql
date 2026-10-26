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
