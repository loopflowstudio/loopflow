-- name: explicit_pr_merge_requests
-- id: 4a9328dff82e9a9a2cb339133e6ed57f
-- depends_on: wave_promotion_occurrence

-- A published PR is ordinary Task continuity until a user or auto merge is
-- explicitly requested for one exact GitHub head.

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
    merge_mode TEXT CHECK (merge_mode IN ('user', 'auto')),
    merge_requested_at INTEGER,
    merge_head_sha TEXT CHECK (merge_head_sha IS NULL OR length(trim(merge_head_sha)) > 0),
    UNIQUE (task_id, sequence),
    CHECK ((github_number IS NULL) = (github_url IS NULL)),
    CHECK (github_number IS NULL OR publication_requested_at IS NOT NULL),
    CHECK (after_merge != 'complete_task' OR next_slug IS NULL),
    CHECK (merge_commit IS NULL OR github_number IS NOT NULL),
    CHECK (merge_commit IS NULL OR abandoned_at IS NULL),
    CHECK ((merge_mode IS NULL) = (merge_requested_at IS NULL)),
    CHECK ((merge_mode IS NULL) = (merge_head_sha IS NULL)),
    CHECK ((merge_mode IS NULL) = (after_merge IS NULL)),
    CHECK (next_slug IS NULL OR merge_mode IS NOT NULL),
    CHECK (merge_mode IS NULL OR github_number IS NOT NULL),
    CHECK (merge_mode IS NULL OR merge_head_sha = github_head_sha)
);

INSERT INTO task_prs_next (
    id, task_id, sequence, slug, branch, base_commit,
    publication_requested_at, after_merge, next_slug,
    github_number, github_url, merge_commit, abandoned_at,
    created_at, updated_at, github_head_sha, ci_observation, parent_pr_id,
    github_observation, linear_attachment_id, linear_comment_id,
    linear_link_error, merge_mode, merge_requested_at, merge_head_sha
)
SELECT
    id, task_id, sequence, slug, branch, base_commit,
    publication_requested_at, NULL, NULL,
    github_number, github_url, merge_commit, abandoned_at,
    created_at, updated_at, github_head_sha, ci_observation, parent_pr_id,
    github_observation, linear_attachment_id, linear_comment_id,
    linear_link_error, NULL, NULL, NULL
FROM task_prs;

DROP TABLE task_prs;
ALTER TABLE task_prs_next RENAME TO task_prs;
CREATE UNIQUE INDEX idx_task_prs_open
    ON task_prs(task_id) WHERE merge_commit IS NULL AND abandoned_at IS NULL;
