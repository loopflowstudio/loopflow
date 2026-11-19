-- A Task may name the Task it was filed from. Children are the Tasks that
-- name a parent; nothing stores a child list.
ALTER TABLE tasks ADD COLUMN parent_task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL;
CREATE INDEX tasks_parent ON tasks(parent_task_id);

DROP TRIGGER store_revision_tasks_update;
CREATE TRIGGER store_revision_tasks_update AFTER UPDATE ON tasks
WHEN NEW.id IS NOT OLD.id OR NEW.repository_id IS NOT OLD.repository_id OR
    NEW.project_id IS NOT OLD.project_id OR
    NEW.parent_task_id IS NOT OLD.parent_task_id OR
    NEW.external_issue_id IS NOT OLD.external_issue_id OR
    NEW.issue_identifier IS NOT OLD.issue_identifier OR
    NEW.created_at IS NOT OLD.created_at OR
    NEW.issue_title IS NOT OLD.issue_title OR
    NEW.issue_description IS NOT OLD.issue_description OR
    NEW.worktree IS NOT OLD.worktree OR
    NEW.workspace_slug IS NOT OLD.workspace_slug OR
    NEW.abandon_reason IS NOT OLD.abandon_reason OR
    NEW.updated_at IS NOT OLD.updated_at OR
    NEW.abandoned_at IS NOT OLD.abandoned_at OR
    NEW.primary_session_id IS NOT OLD.primary_session_id OR
    NEW.planning_revision IS NOT OLD.planning_revision OR
    NEW.planning_rank IS NOT OLD.planning_rank OR
    NEW.planning_assignee IS NOT OLD.planning_assignee OR
    NEW.planning_state IS NOT OLD.planning_state OR
    NEW.planning_completed_at IS NOT OLD.planning_completed_at OR
    NEW.planning_provider_revision IS NOT OLD.planning_provider_revision OR
    NEW.planning_url IS NOT OLD.planning_url OR
    NEW.planning_branch_name IS NOT OLD.planning_branch_name OR
    NEW.planning_team_id IS NOT OLD.planning_team_id OR
    NEW.planning_completed IS NOT OLD.planning_completed OR
    NEW.planning_deleted_at IS NOT OLD.planning_deleted_at OR
    NEW.branch IS NOT OLD.branch OR NEW.base_commit IS NOT OLD.base_commit OR
    NEW.parent_pr_id IS NOT OLD.parent_pr_id OR
    NEW.completion_request IS NOT OLD.completion_request OR
    NEW.completion_error IS NOT OLD.completion_error OR
    NEW.planning_due_date IS NOT OLD.planning_due_date OR
    NEW.checkout_machine_id IS NOT OLD.checkout_machine_id OR
    NEW.paused IS NOT OLD.paused OR NEW.shipped_at IS NOT OLD.shipped_at OR
    NEW.ship_lf_process_id IS NOT OLD.ship_lf_process_id OR
    NEW.ship_restarts IS NOT OLD.ship_restarts OR
    NEW.ship_approved_lf_process_id IS NOT OLD.ship_approved_lf_process_id OR
    NEW.ship_stopped_at IS NOT OLD.ship_stopped_at OR
    NEW.ship_stopped_lf_process_id IS NOT OLD.ship_stopped_lf_process_id OR
    NEW.regressed_in IS NOT OLD.regressed_in OR
    NEW.ship_stopped_person IS NOT OLD.ship_stopped_person OR
    (NEW.pm_snapshot_synced_at IS NULL) IS NOT (OLD.pm_snapshot_synced_at IS NULL)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;

-- A Task's standing is news to its own conversations and to its parent's:
-- its PR merging, the Task closing or being abandoned, and its parent changing.
CREATE TRIGGER task_update_tasks_standing AFTER UPDATE OF planning_completed,abandoned_at,parent_task_id ON tasks
WHEN NEW.planning_completed IS NOT OLD.planning_completed OR NEW.abandoned_at IS NOT OLD.abandoned_at OR NEW.parent_task_id IS NOT OLD.parent_task_id
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER task_update_task_prs_merge AFTER UPDATE OF merge_commit ON task_prs
WHEN NEW.merge_commit IS NOT OLD.merge_commit
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
