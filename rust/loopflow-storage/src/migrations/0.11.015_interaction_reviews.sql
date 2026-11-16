CREATE TABLE interaction_reviews (
    id TEXT PRIMARY KEY NOT NULL,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    project_session_id TEXT NOT NULL REFERENCES project_sessions(id) ON DELETE CASCADE,
    task_session_id TEXT NOT NULL REFERENCES task_sessions(id) ON DELETE CASCADE,
    phase TEXT NOT NULL CHECK (phase IN ('kickoff', 'iterate', 'gate')),
    phase_epoch INTEGER NOT NULL CHECK (phase_epoch > 0),
    flow TEXT NOT NULL,
    step TEXT NOT NULL,
    step_index INTEGER NOT NULL CHECK (step_index >= 0),
    phase_iteration INTEGER NOT NULL CHECK (phase_iteration >= 0),
    policy TEXT NOT NULL CHECK (policy IN ('require', 'defer')),
    reviewer_kind TEXT NOT NULL CHECK (reviewer_kind IN ('human', 'project', 'wave')),
    reviewer_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('requested', 'active', 'completed')),
    reason TEXT NOT NULL,
    prompt TEXT NOT NULL,
    worktree TEXT NOT NULL,
    branch TEXT NOT NULL,
    base_commit TEXT NOT NULL,
    head_commit TEXT NOT NULL,
    worktree_fingerprint TEXT NOT NULL,
    pr_number INTEGER CHECK (pr_number IS NULL OR pr_number > 0),
    pr_url TEXT,
    requested_by_generation INTEGER NOT NULL CHECK (requested_by_generation > 0),
    reviewer_generation INTEGER CHECK (reviewer_generation IS NULL OR reviewer_generation > 0),
    disposition TEXT CHECK (disposition IN ('approved', 'changes_requested')),
    outcome TEXT,
    requested_at INTEGER NOT NULL,
    completed_at INTEGER,
    UNIQUE (task_session_id, phase_epoch, phase_iteration, step_index),
    CHECK ((reviewer_kind = 'human') = (reviewer_id IS NULL)),
    CHECK ((pr_number IS NULL) = (pr_url IS NULL)),
    CHECK (
        (status = 'completed') =
        (disposition IS NOT NULL AND outcome IS NOT NULL AND completed_at IS NOT NULL)
    )
);

CREATE INDEX interaction_reviews_wave_status
    ON interaction_reviews(wave_id, status, requested_at, id);

CREATE INDEX interaction_reviews_task_status
    ON interaction_reviews(task_session_id, status, requested_at, id);
