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

-- A limit the provider scopes to one model (Claude's seven_day_opus) holds
-- back only launches of that model. The window keeps the provider's own name.
ALTER TABLE provider_account_limits ADD COLUMN model TEXT;
ALTER TABLE provider_account_limits ADD COLUMN limited_until INTEGER;

-- A move `lf session move` asked of a busy Session; its Stop hook applies it.
CREATE TABLE session_moves (
    session_id TEXT PRIMARY KEY,
    agent TEXT NOT NULL,
    requested_at INTEGER NOT NULL
);

-- Long-lived Claude tokens are gone: every machine login is a full login or a
-- laptop-renewed access token. The stored tokens are deleted now; their
-- columns stay until the released lf stops reading them (staged drop).
UPDATE provider_accounts SET long_lived_token = NULL, long_lived_token_minted_at = NULL
WHERE long_lived_token IS NOT NULL OR long_lived_token_minted_at IS NOT NULL;
