ALTER TABLE task_sessions
ADD COLUMN pm_snapshot_synced_at INTEGER NOT NULL DEFAULT 0;

ALTER TABLE task_sessions
ADD COLUMN pm_snapshot_warning TEXT;

ALTER TABLE task_sessions
ADD COLUMN pm_writeback_json TEXT NOT NULL DEFAULT '{"state":"current"}';
