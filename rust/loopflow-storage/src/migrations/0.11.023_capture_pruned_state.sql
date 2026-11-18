-- Add the `pruned` terminal capture status: a tombstone for launches whose
-- conversation artifact is known-absent after an explicit `lf runs reconcile`.
-- SQLite bakes the CHECK constraint into the table, so widening the enum
-- requires the documented table rebuild (mirror 0.11.002) rather than an
-- in-place alter. `pruned` reuses the existing `incomplete_reason` column to
-- carry cause + timestamp, so no new column is added.

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
        capture_status IN ('capturing', 'complete', 'partial', 'prompt_only', 'pruned')
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
    task TEXT
);

INSERT INTO agent_launches_next (
    id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
    skill, provider, model, surface, capture_status, incomplete_reason,
    outcome, artifact_dir, conversation_path, provider_events_path,
    provider_session_id, provider_session_path, conversation_event_count,
    conversation_bytes, project, task
)
SELECT
    id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
    skill, provider, model, surface, capture_status, incomplete_reason,
    outcome, artifact_dir, conversation_path, provider_events_path,
    provider_session_id, provider_session_path, conversation_event_count,
    conversation_bytes, project, task
FROM agent_launches;

DROP TABLE agent_launches;
ALTER TABLE agent_launches_next RENAME TO agent_launches;

CREATE INDEX idx_agent_launches_run ON agent_launches(run_id, started_at);
CREATE INDEX idx_agent_launches_process ON agent_launches(process_id, started_at);
CREATE INDEX idx_agent_launches_wave ON agent_launches(wave, started_at);
CREATE INDEX idx_agent_launches_project ON agent_launches(project, started_at);
CREATE INDEX idx_agent_launches_task ON agent_launches(task, started_at);
