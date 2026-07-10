CREATE TABLE pm_snapshots (
    repo TEXT NOT NULL,
    wave TEXT NOT NULL,
    provider TEXT NOT NULL,
    initiative TEXT NOT NULL,
    synced_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    PRIMARY KEY (repo, wave)
);

