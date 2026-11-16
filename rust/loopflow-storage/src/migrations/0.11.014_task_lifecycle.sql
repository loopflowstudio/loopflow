ALTER TABLE task_sessions RENAME COLUMN resolved_flow TO iterate_flow;
ALTER TABLE task_sessions RENAME COLUMN interaction_policy TO iterate_interaction_policy;
ALTER TABLE task_sessions RENAME COLUMN flow_cursor TO phase_cursor;
ALTER TABLE task_sessions RENAME COLUMN flow_iteration TO phase_iteration;

ALTER TABLE task_sessions ADD COLUMN kickoff_flow TEXT NOT NULL DEFAULT 'task-kickoff';
ALTER TABLE task_sessions ADD COLUMN kickoff_interaction_policy TEXT NOT NULL DEFAULT 'require'
    CHECK (kickoff_interaction_policy IN ('require', 'defer'));
ALTER TABLE task_sessions ADD COLUMN gate_flow TEXT NOT NULL DEFAULT 'task-gate';
ALTER TABLE task_sessions ADD COLUMN gate_interaction_policy TEXT NOT NULL DEFAULT 'require'
    CHECK (gate_interaction_policy IN ('require', 'defer'));
ALTER TABLE task_sessions ADD COLUMN lifecycle_phase TEXT NOT NULL DEFAULT 'iterate'
    CHECK (lifecycle_phase IN ('kickoff', 'iterate', 'gate'));
ALTER TABLE task_sessions ADD COLUMN phase_epoch INTEGER NOT NULL DEFAULT 1
    CHECK (phase_epoch > 0);
ALTER TABLE task_sessions ADD COLUMN gate_cycle INTEGER NOT NULL DEFAULT 0
    CHECK (gate_cycle >= 0);
ALTER TABLE task_sessions ADD COLUMN gate_proposal_json TEXT;
