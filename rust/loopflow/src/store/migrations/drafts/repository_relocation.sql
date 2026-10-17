-- Preserve checkout membership when its current address changes. Historical
-- Process paths and attribution remain untouched; this grants no control authority.
CREATE TABLE relocated_task_processes (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    process_id TEXT NOT NULL REFERENCES processes(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, process_id)
);
CREATE INDEX relocated_process_tasks ON relocated_task_processes(process_id, task_id);

CREATE TRIGGER store_revision_relocated_task_processes_insert AFTER INSERT ON relocated_task_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'processes';
END;
CREATE TRIGGER store_revision_relocated_task_processes_update AFTER UPDATE ON relocated_task_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'processes';
END;
CREATE TRIGGER store_revision_relocated_task_processes_delete AFTER DELETE ON relocated_task_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'processes';
END;
