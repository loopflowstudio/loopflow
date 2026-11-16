-- Per Task Session cursor for streaming human Linear edits into Task direction.
-- Records what Linear state has already become a directive or follow-up, so
-- webhook redelivery and out-of-order deliveries never produce a duplicate. One
-- row per Task Session, seeded from the launch snapshot at Session creation and
-- advanced on every applied event.
CREATE TABLE task_linear_observations (
    session_id TEXT PRIMARY KEY REFERENCES task_sessions(id) ON DELETE CASCADE,
    -- Linear issue `updatedAt` last folded in. Monotonic: an observation whose
    -- revision is not newer moves nothing, so a stale/out-of-order response is
    -- dropped rather than replayed.
    last_revision TEXT NOT NULL,
    -- Content basis for the title/description diff. An `updatedAt` bump that
    -- leaves title and description unchanged is a metadata-only edit and ingests
    -- nothing.
    last_title TEXT NOT NULL,
    last_description TEXT NOT NULL,
    -- Unix seconds of the last successful observation, and the degraded reason
    -- when the most recent attempt failed (auth/quota/network). A NULL reason is
    -- healthy; `lf task status` surfaces both.
    last_success_at INTEGER NOT NULL,
    degraded_reason TEXT,
    updated_at INTEGER NOT NULL
);

-- Exactly-once ledger for comment follow-ups. A comment id present here has
-- already been turned into a Task follow-up. A comment becomes a follow-up only
-- on its first insertion into this table, so an at-least-once webhook redelivery
-- enqueues it once.
CREATE TABLE task_linear_ingested_comments (
    session_id TEXT NOT NULL REFERENCES task_sessions(id) ON DELETE CASCADE,
    comment_id TEXT NOT NULL,
    ingested_at INTEGER NOT NULL,
    PRIMARY KEY (session_id, comment_id)
);

-- Backfill existing Task Sessions from their launch snapshot, so a Session that
-- predates this migration diffs its first webhook edit against known content
-- instead of baselining (and swallowing) it. `last_revision` seeds empty so any
-- real Linear `updatedAt` wins the monotonic guard; the comment ledger stays
-- empty because webhooks never deliver a comment that predates subscription.
INSERT INTO task_linear_observations
    (session_id, last_revision, last_title, last_description, last_success_at, degraded_reason, updated_at)
SELECT id, '', issue_title, issue_description, updated_at, NULL, updated_at
FROM task_sessions;
