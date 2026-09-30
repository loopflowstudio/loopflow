-- draft: task_agent
ALTER TABLE tasks ADD COLUMN agent TEXT;

-- draft: task_deletions
-- A positive provider acknowledgement, never inferred from absence or abandonment.
CREATE TABLE task_deletions (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    issue_id TEXT NOT NULL,
    identifier TEXT NOT NULL,
    confirmed_at INTEGER NOT NULL,
    PRIMARY KEY (wave_id, issue_id)
);

-- draft: task_issue_identities
-- Observed identity for removal reconciliation, independent of current planning.
-- An observation proves neither current ownership nor successful deletion.
CREATE TABLE task_issue_identities (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    issue_id TEXT PRIMARY KEY NOT NULL,
    identifier TEXT NOT NULL
);
