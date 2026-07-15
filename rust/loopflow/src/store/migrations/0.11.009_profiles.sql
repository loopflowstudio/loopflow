CREATE TABLE profiles (
    profile_id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE chrome_profile_bindings (
    profile_id TEXT NOT NULL,
    host_id TEXT NOT NULL,
    chrome_directory TEXT NOT NULL,
    google_email TEXT NOT NULL,
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

-- Preserve the account-first router as one profile per account id. These
-- records are intentionally editable: the user can later map several profiles
-- to one reusable provider account without moving its credential home.
INSERT INTO profiles (profile_id, created_at, updated_at)
SELECT account_id, MIN(created_at), MAX(updated_at)
FROM provider_accounts
GROUP BY account_id;

INSERT INTO profile_provider_accounts (
    profile_id, provider, account_id, created_at, updated_at
)
SELECT account_id, provider, account_id, created_at, updated_at
FROM provider_accounts;

UPDATE provider_session_accounts
SET profile_id = account_id
WHERE EXISTS (
    SELECT 1 FROM profiles WHERE profiles.profile_id = provider_session_accounts.account_id
);
