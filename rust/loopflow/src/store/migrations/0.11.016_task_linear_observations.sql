-- Per Task Session cursor for streaming human Linear edits into Task direction.
-- Records what Linear state has already become a directive or follow-up, so
-- restart, polling overlap, and out-of-order responses never produce a
-- duplicate. One row per Task Session, seeded when the Session is first observed
-- (its baseline) and advanced on every later observation.
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
-- already been turned into a Task follow-up, or baselined at first observation.
-- A comment becomes a follow-up only on the transition into this table, so
-- overlapping polls that both see the same comment enqueue it once.
CREATE TABLE task_linear_ingested_comments (
    session_id TEXT NOT NULL REFERENCES task_sessions(id) ON DELETE CASCADE,
    comment_id TEXT NOT NULL,
    ingested_at INTEGER NOT NULL,
    PRIMARY KEY (session_id, comment_id)
);
