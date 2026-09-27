-- name: task_issue_identities
-- id: 01517af294fcda7f536be29e4709530b
-- depends_on: task_deletions

-- Observed identity for removal reconciliation, independent of current planning.
-- An observation proves neither current ownership nor successful deletion.
CREATE TABLE task_issue_identities (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    issue_id TEXT PRIMARY KEY NOT NULL,
    identifier TEXT NOT NULL
);
