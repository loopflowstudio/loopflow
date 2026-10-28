-- A Wave's default graph is committed in `.lf/wave/<name>/config.yaml`;
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

-- Wave setup is part of watched Work projections.
CREATE TRIGGER store_revision_provider_routes_insert AFTER INSERT ON provider_routes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_provider_routes_update AFTER UPDATE ON provider_routes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_provider_routes_delete AFTER DELETE ON provider_routes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

-- Your own overrides of shared setup (`graph`, `agent`, `skill_agents.<skill>`)
-- for a Wave, a repository or every repository, set by `lf set`.
CREATE TABLE scope_settings (
    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('wave', 'repository', 'global')),
    scope_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    set_at INTEGER NOT NULL,
    PRIMARY KEY (scope_kind, scope_id, key),
    CHECK ((scope_kind = 'global') = (scope_id = ''))
);
CREATE TRIGGER store_revision_scope_settings_insert AFTER INSERT ON scope_settings
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_scope_settings_update AFTER UPDATE ON scope_settings
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_scope_settings_delete AFTER DELETE ON scope_settings
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
