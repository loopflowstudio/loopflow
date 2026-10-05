-- draft: shared_provider_homes
-- Conversations pinned before shared provider homes live in their account's
-- own home, so existing rows are isolated. Every new pin states its mode.
ALTER TABLE provider_session_accounts ADD COLUMN isolated INTEGER NOT NULL DEFAULT 1
    CHECK (isolated IN (0, 1));

-- Every change of the account a provider's native home is signed in as.
-- Usage attribution for shared agents reads this, not a launch's named account.
CREATE TABLE provider_account_switches (
    id INTEGER PRIMARY KEY,
    provider TEXT NOT NULL,
    account_id TEXT NOT NULL,
    switched_at INTEGER NOT NULL,
    cause TEXT NOT NULL CHECK (cause IN ('person', 'exhaustion'))
);

CREATE INDEX idx_provider_account_switches_provider_time
    ON provider_account_switches(provider, switched_at);
