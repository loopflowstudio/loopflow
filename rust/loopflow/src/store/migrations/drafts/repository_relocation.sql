-- Preserve checkout membership when its current address changes. Historical
-- Process paths and attribution remain untouched; this grants no control authority.
CREATE TABLE relocated_task_processes (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    process_id TEXT NOT NULL REFERENCES processes(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, process_id)
);
CREATE INDEX relocated_process_tasks ON relocated_task_processes(process_id, task_id);
