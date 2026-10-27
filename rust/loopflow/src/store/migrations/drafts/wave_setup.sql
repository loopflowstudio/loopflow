-- A Wave's default Workflow is committed in `.lf/wave/<name>/config.yaml`;
-- the local selection it replaces is dropped.
ALTER TABLE waves DROP COLUMN workflow;

-- Account routes gain a Wave scope, between repository and default.
CREATE TABLE provider_routes_next (
    scope TEXT NOT NULL CHECK (scope IN ('wave', 'repo', 'default')),
    scope_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    account_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (scope, scope_id, provider, position),
    UNIQUE (scope, scope_id, provider, account_id),
    CHECK (
        (scope = 'default' AND scope_id = '')
        OR (scope IN ('wave', 'repo') AND scope_id <> '')
    ),
    FOREIGN KEY (provider, account_id)
        REFERENCES provider_accounts(provider, account_id) ON DELETE RESTRICT
);
INSERT INTO provider_routes_next SELECT
    scope, scope_id, provider, position, account_id, created_at, updated_at
FROM provider_routes;
DROP TABLE provider_routes;
ALTER TABLE provider_routes_next RENAME TO provider_routes;
