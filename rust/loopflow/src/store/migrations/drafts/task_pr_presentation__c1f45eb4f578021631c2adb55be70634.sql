-- name: task_pr_presentation
-- id: c1f45eb4f578021631c2adb55be70634
-- depends_on:
ALTER TABLE task_prs ADD COLUMN pr_title TEXT;
ALTER TABLE task_prs ADD COLUMN pr_body TEXT;
ALTER TABLE task_prs ADD COLUMN pr_copy_head_sha TEXT;
