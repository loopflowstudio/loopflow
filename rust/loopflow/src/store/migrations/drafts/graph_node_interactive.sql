-- A Graph step's `human` flag is now `interactive`. Recorded Graphs keep the old
-- key, which no longer reads, so it is renamed in every step, XOR paths included.
-- Inside JSON strings a quote is escaped, so `,"human":` matches only the key.
-- The record is append-only, so its trigger steps aside for the rewrite.
DROP TRIGGER graph_processes_are_append_only;
UPDATE graph_processes SET compiled=replace(compiled,',"human":',',"interactive":')
WHERE instr(compiled,',"human":')>0;
CREATE TRIGGER graph_processes_are_append_only BEFORE UPDATE ON graph_processes BEGIN
    SELECT RAISE(ABORT,'A Graph record is append-only');
END;

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
