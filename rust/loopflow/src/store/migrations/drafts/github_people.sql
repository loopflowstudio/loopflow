-- A Task owner's Git email resolves once to a GitHub user through the
-- repository's commits. A NULL `login` records that GitHub knows no user for the
-- email; that miss is retried after a day.
CREATE TABLE github_people(
    email TEXT PRIMARY KEY,
    login TEXT,
    name TEXT,
    avatar_url TEXT,
    checked_at INTEGER NOT NULL,
    CHECK ((login IS NULL) = (avatar_url IS NULL))
);
