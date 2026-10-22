-- `lf ship` approves a Task by appending a `shipped` move. SQLite cannot widen a
-- CHECK in place, so the history table is rebuilt with its rows, index and triggers.
CREATE TABLE task_workflow_moves_shipped (
    seq INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    workflow TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('took_up', 'chose', 'arrived', 'set', 'shipped')),
    from_node TEXT NOT NULL,
    to_node TEXT NOT NULL,
    edge INTEGER,
    "lf_process_id" TEXT REFERENCES "processes"("id"),
    note TEXT,
    at INTEGER NOT NULL
) STRICT;
INSERT INTO task_workflow_moves_shipped(seq,task_id,workflow,kind,from_node,to_node,edge,lf_process_id,note,at)
SELECT seq,task_id,workflow,kind,from_node,to_node,edge,lf_process_id,note,at FROM task_workflow_moves;
DROP TABLE task_workflow_moves;
ALTER TABLE task_workflow_moves_shipped RENAME TO task_workflow_moves;
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
