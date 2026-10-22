-- draft: task_paused
ALTER TABLE tasks ADD COLUMN paused INTEGER NOT NULL DEFAULT 0 CHECK (paused IN (0,1));
