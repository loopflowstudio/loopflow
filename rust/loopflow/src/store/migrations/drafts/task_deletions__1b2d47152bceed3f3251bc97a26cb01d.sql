-- name: task_deletions
-- id: 1b2d47152bceed3f3251bc97a26cb01d
-- depends_on: 

-- A positive provider acknowledgement, never inferred from absence or abandonment.
CREATE TABLE task_deletions (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    issue_id TEXT NOT NULL,
    identifier TEXT NOT NULL,
    confirmed_at INTEGER NOT NULL,
    PRIMARY KEY (wave_id, issue_id)
);
