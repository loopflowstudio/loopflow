-- Keep three terminal capture facts distinct:
--   pruned      the referenced conversation is known absent
--   interrupted retained evidence outlived its owner
--   lost        retained evidence has an acknowledged write gap
-- SQLite bakes the capture_status CHECK into agent_launches, so widening the
-- closed state set requires rebuilding the table and restoring every index.

CREATE TABLE agent_launches_next (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    process_id TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
    repo TEXT NOT NULL,
    worktree TEXT NOT NULL,
    wave TEXT,
    flow TEXT,
    skill TEXT,
    provider TEXT NOT NULL,
    model TEXT,
    surface TEXT NOT NULL,
    capture_status TEXT NOT NULL CHECK (
        capture_status IN (
            'capturing', 'complete', 'partial', 'prompt_only',
            'pruned', 'interrupted', 'lost'
        )
    ),
    incomplete_reason TEXT,
    outcome TEXT NOT NULL CHECK (
        outcome IN ('running', 'completed', 'failed', 'interrupted')
    ),
    artifact_dir TEXT NOT NULL,
    conversation_path TEXT NOT NULL,
    provider_events_path TEXT,
    provider_session_id TEXT,
    provider_session_path TEXT,
    conversation_event_count INTEGER NOT NULL,
    conversation_bytes INTEGER NOT NULL,
    project TEXT,
    task TEXT,
    product_run_id TEXT REFERENCES runs(id) ON DELETE RESTRICT,
    home_id TEXT REFERENCES homes(id) ON DELETE RESTRICT,
    account_id TEXT,
    launch_state TEXT CHECK (
        launch_state IN ('starting', 'live', 'stopping', 'ended')
    ),
    containment_kind TEXT CHECK (
        containment_kind IN ('process_group', 'tmux')
    ),
    containment_id TEXT,
    resume_token TEXT,
    opaque_epoch_id TEXT REFERENCES epochs(id) ON DELETE RESTRICT,
    opaque_basis_rev INTEGER,
    attention_kind TEXT CHECK (
        attention_kind IN ('user', 'parent')
    ),
    attention_work_kind TEXT CHECK (
        attention_work_kind IN ('wave', 'project', 'task')
    ),
    attention_work_id TEXT,
    attention_at INTEGER,
    handback_state TEXT CHECK (
        handback_state IN ('succeeded', 'failed', 'interrupted', 'unknown')
    )
);

INSERT INTO agent_launches_next (
    id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
    skill, provider, model, surface, capture_status, incomplete_reason,
    outcome, artifact_dir, conversation_path, provider_events_path,
    provider_session_id, provider_session_path, conversation_event_count,
    conversation_bytes, project, task, product_run_id, home_id, account_id,
    launch_state, containment_kind, containment_id, resume_token,
    opaque_epoch_id, opaque_basis_rev, attention_kind, attention_work_kind,
    attention_work_id, attention_at, handback_state
)
SELECT
    id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
    skill, provider, model, surface, capture_status, incomplete_reason,
    outcome, artifact_dir, conversation_path, provider_events_path,
    provider_session_id, provider_session_path, conversation_event_count,
    conversation_bytes, project, task, product_run_id, home_id, account_id,
    launch_state, containment_kind, containment_id, resume_token,
    opaque_epoch_id, opaque_basis_rev, attention_kind, attention_work_kind,
    attention_work_id, attention_at, handback_state
FROM agent_launches;

DROP TABLE agent_launches;
ALTER TABLE agent_launches_next RENAME TO agent_launches;

CREATE INDEX idx_agent_launches_run ON agent_launches(run_id, started_at);
CREATE INDEX idx_agent_launches_process ON agent_launches(process_id, started_at);
CREATE INDEX idx_agent_launches_wave ON agent_launches(wave, started_at);
CREATE INDEX idx_agent_launches_project ON agent_launches(project, started_at);
CREATE INDEX idx_agent_launches_task ON agent_launches(task, started_at);
CREATE UNIQUE INDEX idx_agent_launches_one_control_live
    ON agent_launches(product_run_id)
    WHERE launch_state IN ('starting', 'live', 'stopping');
CREATE INDEX idx_agent_launches_attention
    ON agent_launches(attention_kind, attention_work_kind, attention_work_id, attention_at)
    WHERE attention_kind IS NOT NULL;
