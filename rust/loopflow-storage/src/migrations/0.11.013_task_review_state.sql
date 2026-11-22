ALTER TABLE task_sessions ADD COLUMN resolved_flow TEXT NOT NULL DEFAULT 'task';
ALTER TABLE task_sessions ADD COLUMN interaction_policy TEXT NOT NULL DEFAULT 'require'
    CHECK (interaction_policy IN ('require', 'defer'));
ALTER TABLE task_sessions ADD COLUMN flow_cursor INTEGER NOT NULL DEFAULT 0
    CHECK (flow_cursor >= 0);
ALTER TABLE task_sessions ADD COLUMN flow_iteration INTEGER NOT NULL DEFAULT 0
    CHECK (flow_iteration >= 0);
