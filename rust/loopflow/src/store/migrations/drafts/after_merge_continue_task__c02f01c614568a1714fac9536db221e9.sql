-- name: after_merge_continue_task
-- id: c02f01c614568a1714fac9536db221e9
-- depends_on:

-- A non-terminal Task PR continues the Task; it does not create a review gate.

CREATE TABLE task_prs_next (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 1),
    slug TEXT NOT NULL,
    branch TEXT NOT NULL UNIQUE,
    base_commit TEXT NOT NULL,
    publication_requested_at INTEGER,
    after_merge TEXT CHECK (after_merge IN ('continue_task', 'complete_task')),
    next_slug TEXT,
    github_number INTEGER CHECK (github_number > 0),
    github_url TEXT,
    merge_commit TEXT,
    abandoned_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    github_head_sha TEXT,
    ci_observation TEXT,
    parent_pr_id TEXT REFERENCES task_prs_next(id),
    github_observation TEXT,
    linear_attachment_id TEXT,
    linear_comment_id TEXT,
    linear_link_error TEXT,
    UNIQUE (task_id, sequence),
    CHECK ((publication_requested_at IS NULL) = (after_merge IS NULL)),
    CHECK ((github_number IS NULL) = (github_url IS NULL)),
    CHECK (github_number IS NULL OR publication_requested_at IS NOT NULL),
    CHECK (after_merge != 'complete_task' OR next_slug IS NULL),
    CHECK (merge_commit IS NULL OR github_number IS NOT NULL),
    CHECK (merge_commit IS NULL OR abandoned_at IS NULL)
);

INSERT INTO task_prs_next (
    id, task_id, sequence, slug, branch, base_commit,
    publication_requested_at, after_merge, next_slug,
    github_number, github_url, merge_commit, abandoned_at,
    created_at, updated_at, github_head_sha, ci_observation, parent_pr_id,
    github_observation, linear_attachment_id, linear_comment_id,
    linear_link_error
)
SELECT
    id, task_id, sequence, slug, branch, base_commit,
    publication_requested_at,
    CASE after_merge WHEN 'review' THEN 'continue_task' ELSE after_merge END,
    next_slug, github_number, github_url, merge_commit, abandoned_at,
    created_at, updated_at, github_head_sha, ci_observation, parent_pr_id,
    github_observation, linear_attachment_id, linear_comment_id,
    linear_link_error
FROM task_prs;

DROP TABLE task_prs;
ALTER TABLE task_prs_next RENAME TO task_prs;
CREATE UNIQUE INDEX idx_task_prs_open
    ON task_prs(task_id) WHERE merge_commit IS NULL AND abandoned_at IS NULL;
