-- Ownership columns before settings_authority moved Machine routes to files.
CREATE TABLE machines (id TEXT PRIMARY KEY, route TEXT NOT NULL UNIQUE);
INSERT INTO machines VALUES ('local-machine', 'local'), ('remote-machine', 'ssh://remote');
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    worktree TEXT,
    abandoned_at INTEGER,
    planning_completed INTEGER,
    checkout_machine_id TEXT
);
INSERT INTO tasks VALUES
    ('local-done', '/repo.local-done', NULL, 1, 'local-machine'),
    ('remote-done', '/repo.remote-done', NULL, 1, 'remote-machine'),
    ('running', '/repo.running', NULL, NULL, 'local-machine'),
    ('unknown', '/repo.unknown', NULL, NULL, NULL);
