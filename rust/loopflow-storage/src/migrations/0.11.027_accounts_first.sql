CREATE TABLE access_profiles (
    profile_id TEXT PRIMARY KEY,
    chrome_directory TEXT NOT NULL UNIQUE,
    expected_login TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

INSERT INTO access_profiles (
    profile_id, chrome_directory, expected_login, created_at, updated_at
)
SELECT
    profiles.profile_id,
    chrome_profile_bindings.chrome_directory,
    profiles.profile_id,
    MIN(profiles.created_at, chrome_profile_bindings.created_at),
    MAX(profiles.updated_at, chrome_profile_bindings.updated_at)
FROM profiles
JOIN chrome_profile_bindings USING (profile_id);

CREATE TABLE account_access_profiles (
    provider TEXT NOT NULL,
    account_id TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    profile_id TEXT NOT NULL,
    PRIMARY KEY (provider, account_id, position),
    UNIQUE (provider, account_id, profile_id),
    FOREIGN KEY (provider, account_id)
        REFERENCES provider_accounts(provider, account_id) ON DELETE CASCADE,
    FOREIGN KEY (profile_id)
        REFERENCES access_profiles(profile_id) ON DELETE RESTRICT
);

INSERT INTO account_access_profiles (provider, account_id, position, profile_id)
SELECT provider, account_id, position, profile_id
FROM (
    SELECT
        mapping.provider,
        mapping.account_id,
        mapping.profile_id,
        row_number() OVER (
            PARTITION BY mapping.provider, mapping.account_id
            ORDER BY
                CASE
                    WHEN lower(mapping.profile_id) = lower(account.login_email) THEN 0
                    ELSE 1
                END,
                mapping.updated_at,
                mapping.profile_id
        ) - 1 AS position
    FROM profile_provider_accounts AS mapping
    JOIN access_profiles USING (profile_id)
    JOIN provider_accounts AS account
      ON account.provider = mapping.provider
     AND account.account_id = mapping.account_id
);

CREATE TABLE provider_routes (
    scope TEXT NOT NULL CHECK (scope IN ('repo', 'default')),
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
        OR (scope = 'repo' AND scope_id <> '')
    ),
    FOREIGN KEY (provider, account_id)
        REFERENCES provider_accounts(provider, account_id) ON DELETE RESTRICT
);

WITH ordered_profiles AS (
    SELECT
        route.repo_id,
        route.default_profile AS profile_id,
        0 AS position,
        route.created_at,
        route.updated_at
    FROM repo_profile_routes AS route
    UNION ALL
    SELECT
        backup.repo_id,
        backup.profile_id,
        backup.position + 1,
        route.created_at,
        route.updated_at
    FROM repo_backup_profiles AS backup
    JOIN repo_profile_routes AS route USING (repo_id)
),
mapped_accounts AS (
    SELECT
        ordered.repo_id,
        mapping.provider,
        mapping.account_id,
        ordered.position,
        ordered.created_at,
        ordered.updated_at,
        row_number() OVER (
            PARTITION BY ordered.repo_id, mapping.provider, mapping.account_id
            ORDER BY ordered.position
        ) AS account_occurrence
    FROM ordered_profiles AS ordered
    JOIN profile_provider_accounts AS mapping USING (profile_id)
),
deduplicated AS (
    SELECT
        repo_id,
        provider,
        account_id,
        created_at,
        updated_at,
        row_number() OVER (
            PARTITION BY repo_id, provider
            ORDER BY position
        ) - 1 AS position
    FROM mapped_accounts
    WHERE account_occurrence = 1
)
INSERT INTO provider_routes (
    scope, scope_id, provider, position, account_id, created_at, updated_at
)
SELECT
    'repo', repo_id, provider, position, account_id, created_at, updated_at
FROM deduplicated;

INSERT INTO provider_routes (
    scope, scope_id, provider, position, account_id, created_at, updated_at
)
SELECT
    'default', '', provider,
    row_number() OVER (
        PARTITION BY provider
        ORDER BY last_selected_at IS NULL, last_selected_at DESC, account_id
    ) - 1,
    account_id,
    created_at,
    updated_at
FROM provider_accounts
WHERE routing_state = 'automatic';

CREATE TABLE provider_session_accounts_new (
    provider TEXT NOT NULL,
    provider_session_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (provider, provider_session_id),
    FOREIGN KEY (provider, account_id)
        REFERENCES provider_accounts(provider, account_id) ON DELETE CASCADE
);

INSERT INTO provider_session_accounts_new (
    provider, provider_session_id, account_id, created_at
)
SELECT provider, provider_session_id, account_id, created_at
FROM provider_session_accounts;

DROP TABLE provider_session_accounts;
ALTER TABLE provider_session_accounts_new RENAME TO provider_session_accounts;

DROP TABLE repo_backup_profiles;
DROP TABLE repo_profile_routes;
DROP TABLE profile_provider_accounts;
DROP TABLE chrome_profile_bindings;
DROP TABLE profiles;
