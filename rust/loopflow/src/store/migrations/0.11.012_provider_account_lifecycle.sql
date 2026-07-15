DROP INDEX idx_provider_accounts_preferred;

CREATE TABLE provider_accounts_lifecycle (
    provider TEXT NOT NULL,
    account_id TEXT NOT NULL,
    home TEXT,
    login_email TEXT,
    credential_state TEXT NOT NULL CHECK (
        credential_state IN ('connected', 'missing')
    ),
    routing_state TEXT NOT NULL CHECK (
        routing_state IN ('automatic', 'explicit_only', 'disabled')
    ),
    plan TEXT,
    paid_through INTEGER,
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

INSERT INTO provider_accounts_lifecycle (
    provider, account_id, home, login_email, credential_state, routing_state,
    plan, paid_through, utilization_percent, cooldown_until, cooldown_reason,
    last_selected_at, created_at, updated_at
)
SELECT
    provider,
    account_id,
    home,
    CASE
        WHEN login IS NULL THEN NULL
        WHEN account_id = (
            SELECT candidate.account_id
            FROM provider_accounts AS candidate
            WHERE candidate.provider = provider_accounts.provider
              AND lower(candidate.login) = lower(provider_accounts.login)
            ORDER BY candidate.preferred DESC,
                     candidate.updated_at DESC,
                     candidate.account_id
            LIMIT 1
        ) THEN lower(login)
        ELSE NULL
    END,
    CASE WHEN home IS NULL THEN 'missing' ELSE 'connected' END,
    CASE WHEN enabled = 1 THEN 'automatic' ELSE 'disabled' END,
    NULL,
    NULL,
    utilization_percent,
    cooldown_until,
    cooldown_reason,
    last_selected_at,
    created_at,
    updated_at
FROM provider_accounts;

DROP TABLE provider_accounts;
ALTER TABLE provider_accounts_lifecycle RENAME TO provider_accounts;

CREATE UNIQUE INDEX idx_provider_accounts_login_email
ON provider_accounts(provider, login_email)
WHERE login_email IS NOT NULL;
