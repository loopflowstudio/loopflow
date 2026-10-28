-- draft: forget_fly_rentals
-- Fly is no longer a rental provider, and no reader parses `fly`. 0.14.17
-- could write Fly on-demands and rentals, so they go, along with any scope
-- that sent its Tasks to one. A Fly app left behind bills only its IPv4 until
-- `fly apps destroy` removes it.
DELETE FROM rentals WHERE provider='fly';
UPDATE scope_defaults SET tasks_capacity = NULL
    WHERE tasks_capacity IN (SELECT name FROM ondemands WHERE provider='fly');
DELETE FROM ondemands WHERE provider='fly';

-- draft: github_people
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

-- draft: session_moves
-- A Session moved to another Machine is completed here and names the Machine
-- it went to, so resuming it here follows it there.
ALTER TABLE agent_sessions ADD COLUMN moved_to_machine_id TEXT;
