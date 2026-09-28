-- name: auth_browser_bindings
-- id: 2cd101ad721f80ca7903d743acc094c4
-- depends_on: 

CREATE TABLE auth_browser_bindings (
    provider TEXT NOT NULL,
    account_id TEXT,
    position INTEGER NOT NULL CHECK (position >= 0),
    profile_id TEXT NOT NULL,
    FOREIGN KEY (provider, account_id) REFERENCES provider_accounts(provider, account_id) ON DELETE CASCADE,
    FOREIGN KEY (profile_id) REFERENCES access_profiles(profile_id) ON DELETE RESTRICT
);
CREATE UNIQUE INDEX auth_browser_position ON auth_browser_bindings(provider, coalesce(account_id, ''), position);
CREATE UNIQUE INDEX auth_browser_profile ON auth_browser_bindings(provider, coalesce(account_id, ''), profile_id);
INSERT INTO auth_browser_bindings SELECT provider, account_id, position, profile_id FROM account_access_profiles;
DROP TABLE account_access_profiles;

-- Rebuild the profile owner to allow Chrome profiles without a Google login.
CREATE TABLE access_profiles_nullable (
    profile_id TEXT PRIMARY KEY,
    chrome_directory TEXT NOT NULL UNIQUE,
    expected_login TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
INSERT INTO access_profiles_nullable SELECT * FROM access_profiles;
DROP TABLE access_profiles;
ALTER TABLE access_profiles_nullable RENAME TO access_profiles;
