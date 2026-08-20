-- draft: task_lifecycle_contract
ALTER TABLE tasks ADD COLUMN lifecycle_outcome TEXT NOT NULL DEFAULT 'delivery'
    CHECK (lifecycle_outcome IN ('delivery', 'design_only'));

ALTER TABLE task_prs ADD COLUMN pr_title TEXT;
ALTER TABLE task_prs ADD COLUMN pr_body TEXT;
ALTER TABLE task_prs ADD COLUMN pr_copy_head_sha TEXT;
