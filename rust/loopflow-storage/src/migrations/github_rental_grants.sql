-- One row per GitHub repository whose rentals this Machine signs in: the
-- GitHub App user token they hold and the grant that renews it. Both tokens
-- are encrypted with the provider token key.
CREATE TABLE github_rental_grants (
    repository_id INTEGER PRIMARY KEY NOT NULL,
    repository TEXT NOT NULL,
    login TEXT NOT NULL,
    access_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    expires_at INTEGER NOT NULL
) STRICT;
