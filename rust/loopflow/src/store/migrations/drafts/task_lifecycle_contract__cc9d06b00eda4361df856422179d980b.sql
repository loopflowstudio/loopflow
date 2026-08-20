-- name: task_lifecycle_contract
-- id: cc9d06b00eda4361df856422179d980b
-- depends_on:
ALTER TABLE tasks ADD COLUMN lifecycle_outcome TEXT NOT NULL DEFAULT 'delivery'
    CHECK (lifecycle_outcome IN ('delivery', 'design_only'));

ALTER TABLE task_prs ADD COLUMN pr_title TEXT;
ALTER TABLE task_prs ADD COLUMN pr_body TEXT;
ALTER TABLE task_prs ADD COLUMN pr_copy_head_sha TEXT;
