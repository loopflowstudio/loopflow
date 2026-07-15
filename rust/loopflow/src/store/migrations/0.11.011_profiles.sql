CREATE TABLE profiles (
    profile_id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE chrome_profile_bindings (
    profile_id TEXT NOT NULL,
    host_id TEXT NOT NULL,
    chrome_directory TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (profile_id, host_id),
    UNIQUE (host_id, chrome_directory),
    FOREIGN KEY (profile_id) REFERENCES profiles(profile_id) ON DELETE CASCADE
);

CREATE TABLE profile_provider_accounts (
    profile_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    account_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (profile_id, provider),
    FOREIGN KEY (profile_id) REFERENCES profiles(profile_id) ON DELETE CASCADE,
    FOREIGN KEY (provider, account_id)
        REFERENCES provider_accounts(provider, account_id)
        ON DELETE RESTRICT
);

CREATE TABLE repo_profile_routes (
    repo_id TEXT PRIMARY KEY,
    default_profile TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (default_profile) REFERENCES profiles(profile_id) ON DELETE RESTRICT
);

CREATE TABLE repo_backup_profiles (
    repo_id TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    profile_id TEXT NOT NULL,
    PRIMARY KEY (repo_id, position),
    UNIQUE (repo_id, profile_id),
    FOREIGN KEY (repo_id) REFERENCES repo_profile_routes(repo_id) ON DELETE CASCADE,
    FOREIGN KEY (profile_id) REFERENCES profiles(profile_id) ON DELETE RESTRICT
);

ALTER TABLE provider_session_accounts ADD COLUMN profile_id TEXT
    REFERENCES profiles(profile_id) ON DELETE CASCADE;

-- Preserve verified provider logins as email-identified profiles. Account ids
-- remain opaque credential-home keys and never become profile identity.
INSERT INTO profiles (profile_id, created_at, updated_at)
SELECT lower(login), MIN(created_at), MAX(updated_at)
FROM provider_accounts
WHERE login IS NOT NULL AND instr(login, '@') > 1
GROUP BY lower(login);

INSERT INTO profile_provider_accounts (
    profile_id, provider, account_id, created_at, updated_at
)
SELECT profile_id, provider, account_id, created_at, updated_at
FROM (
    SELECT
        lower(login) AS profile_id,
        provider,
        account_id,
        created_at,
        updated_at,
        row_number() OVER (
            PARTITION BY provider, lower(login)
            ORDER BY preferred DESC, updated_at DESC, account_id
        ) AS preference_rank
    FROM provider_accounts
    WHERE login IS NOT NULL AND instr(login, '@') > 1
)
WHERE preference_rank = 1;

UPDATE provider_session_accounts
SET profile_id = (
    SELECT lower(provider_accounts.login)
    FROM provider_accounts
    WHERE provider_accounts.provider = provider_session_accounts.provider
      AND provider_accounts.account_id = provider_session_accounts.account_id
)
WHERE EXISTS (
    SELECT 1
    FROM provider_accounts
    WHERE provider_accounts.provider = provider_session_accounts.provider
      AND provider_accounts.account_id = provider_session_accounts.account_id
      AND provider_accounts.login IS NOT NULL
      AND instr(provider_accounts.login, '@') > 1
);
