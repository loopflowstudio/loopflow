-- name: ask_linear_comment_outbox
-- id: ec34e3ebbdca1899f5aacd7c89a3a271
-- depends_on: durable_asks

-- Ask and Answer commits enqueue their Linear write in the same transaction.
-- The provider call happens afterward; attempt state makes failures visible and
-- lets a later command reconcile a remotely-created comment without duplicating
-- it after a local process crash.
CREATE TABLE ask_linear_comment_outbox (
    ask_id TEXT NOT NULL REFERENCES ask_exchanges(id) ON DELETE RESTRICT,
    transition TEXT NOT NULL CHECK (transition IN ('ask', 'answer')),
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
    issue_id TEXT NOT NULL CHECK (length(trim(issue_id)) > 0),
    body TEXT NOT NULL CHECK (length(trim(body)) > 0),
    created_at INTEGER NOT NULL,
    attempt_count INTEGER NOT NULL CHECK (attempt_count >= 0),
    attempt_started_at INTEGER,
    last_error TEXT,
    linear_comment_id TEXT,
    delivered_at INTEGER,
    PRIMARY KEY (ask_id, transition),
    CHECK (
        (linear_comment_id IS NULL AND delivered_at IS NULL)
        OR
        (linear_comment_id IS NOT NULL
         AND length(trim(linear_comment_id)) > 0
         AND delivered_at IS NOT NULL)
    )
);
CREATE INDEX idx_ask_linear_comment_outbox_pending
    ON ask_linear_comment_outbox(delivered_at, created_at, ask_id, transition);
