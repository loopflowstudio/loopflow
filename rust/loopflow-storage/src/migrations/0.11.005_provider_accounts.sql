CREATE TABLE provider_accounts (
    provider TEXT NOT NULL,
    account_id TEXT NOT NULL,
    home TEXT,
    login TEXT,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    preferred INTEGER NOT NULL CHECK (preferred IN (0, 1)),
    utilization_percent INTEGER CHECK (
        utilization_percent IS NULL OR
        utilization_percent BETWEEN 0 AND 100
    ),
    cooldown_until INTEGER,
    cooldown_reason TEXT,
    last_selected_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (provider, account_id)
);
CREATE UNIQUE INDEX idx_provider_accounts_preferred
ON provider_accounts(provider)
WHERE preferred = 1;

CREATE TABLE provider_session_accounts (
    provider TEXT NOT NULL,
    provider_session_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (provider, provider_session_id),
    FOREIGN KEY (provider, account_id)
        REFERENCES provider_accounts(provider, account_id)
        ON DELETE CASCADE
);
