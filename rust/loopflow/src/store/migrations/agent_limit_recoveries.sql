-- How a headless step Process is recovering from a usage limit. `retry_at` set:
-- nothing can launch before then. NULL: it went on under another account.
CREATE TABLE agent_limit_recoveries (
    lf_process_id TEXT PRIMARY KEY REFERENCES processes(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    reason TEXT NOT NULL,
    retry_at INTEGER,
    updated_at INTEGER NOT NULL
);

-- Task execution reads it.
CREATE TRIGGER store_revision_agent_limit_recoveries_insert AFTER INSERT ON agent_limit_recoveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='processes'; END;
CREATE TRIGGER store_revision_agent_limit_recoveries_update AFTER UPDATE ON agent_limit_recoveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='processes'; END;
CREATE TRIGGER store_revision_agent_limit_recoveries_delete AFTER DELETE ON agent_limit_recoveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='processes'; END;
