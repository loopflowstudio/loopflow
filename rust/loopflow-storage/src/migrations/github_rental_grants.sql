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

-- The GitHub repository a rental is signed in for, stored because a rental
-- made by hand has no Task to derive it from, and the expiry of the token it
-- was last sent. A rental whose expiry is not its repository's current
-- `github_rental_grants.expires_at` holds a token that stopped working.
ALTER TABLE rentals ADD COLUMN github_repository_id INTEGER;
ALTER TABLE rentals ADD COLUMN github_token_expires_at INTEGER;
