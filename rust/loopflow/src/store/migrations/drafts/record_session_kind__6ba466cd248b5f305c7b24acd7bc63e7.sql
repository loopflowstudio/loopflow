-- name: record_session_kind
-- id: 6ba466cd248b5f305c7b24acd7bc63e7
-- depends_on: record_invocation_attempts

-- Every Session stored before this draft is a Task review. Writers always name
-- the kind; the column default exists only to convert those rows.
ALTER TABLE sessions ADD COLUMN kind TEXT NOT NULL DEFAULT 'flow_review'
    CHECK (kind IN ('interactive', 'flow_review', 'ask'));

-- A Session reads its provider through its current Run. Historical review
-- Runs keep theirs in launch evidence until the offline import fills it.
ALTER TABLE runs ADD COLUMN provider TEXT;
ALTER TABLE runs ADD COLUMN model TEXT;
