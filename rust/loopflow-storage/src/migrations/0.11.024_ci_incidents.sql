-- ci_incidents
-- Historical CI recovery evidence across PR heads.
--
-- TaskPr keeps only the current check observation. An incident preserves the
-- failure-to-response-to-green-to-merge milestones after that current row
-- advances, without becoming another wake queue.
CREATE TABLE ci_incidents (
    identity              TEXT    PRIMARY KEY,
    task_session_id       TEXT    NOT NULL REFERENCES task_sessions(id) ON DELETE CASCADE,
    pr_id                  TEXT    NOT NULL REFERENCES task_prs(id) ON DELETE CASCADE,
    repo                   TEXT    NOT NULL,
    pr_number              INTEGER NOT NULL CHECK (pr_number > 0),
    failed_head_sha        TEXT    NOT NULL,
    failure_set_json       TEXT    NOT NULL,
    provider_completed_at  INTEGER,
    poll_observed_at       INTEGER,
    webhook_received_at    INTEGER,
    trigger_command_id     TEXT,
    responded_at           INTEGER,
    green_at               INTEGER,
    merged_at              INTEGER,
    blocked_at             INTEGER,
    blocked_reason         TEXT,
    created_at             INTEGER NOT NULL,
    updated_at             INTEGER NOT NULL,
    CHECK (poll_observed_at IS NOT NULL OR webhook_received_at IS NOT NULL),
    CHECK ((blocked_at IS NULL) = (blocked_reason IS NULL))
);

CREATE INDEX idx_ci_incidents_observed
    ON ci_incidents(poll_observed_at, webhook_received_at);
CREATE INDEX idx_ci_incidents_pr
    ON ci_incidents(pr_id, created_at);
CREATE INDEX idx_ci_incidents_open
    ON ci_incidents(green_at, merged_at, updated_at);
