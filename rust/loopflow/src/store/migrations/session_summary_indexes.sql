-- Every Session summary reads its latest provider transition. Those rows are
-- observations whose receipt keys name them (`provider-transition:` now,
-- `agent-session:` in released history), so a partial index over just them
-- replaces a JSON scan of the Session's whole history.
CREATE INDEX session_provider_transitions ON session_events(session_id, seq)
WHERE kind = 'observed'
    AND (receipt_key GLOB 'provider-transition:*' OR receipt_key GLOB 'agent-session:*');

-- A reading time is not a change. An update that only moves an observation
-- stamp, or rewrites a row with the values it already holds (a retried
-- delivery saving the same error), leaves planning readers alone. A stamp's
-- first value still counts: acquisition derives its state from it.

DROP TRIGGER store_revision_pm_items_update;
CREATE TRIGGER store_revision_pm_items_update AFTER UPDATE ON pm_items
WHEN NEW.repo IS NOT OLD.repo OR NEW.provider IS NOT OLD.provider OR
    NEW.id IS NOT OLD.id OR NEW.identifier IS NOT OLD.identifier OR
    NEW.project_id IS NOT OLD.project_id OR
    NEW.needs_refresh IS NOT OLD.needs_refresh OR
    NEW.revision IS NOT OLD.revision OR NEW.branch_name IS NOT OLD.branch_name OR
    NEW.url IS NOT OLD.url OR NEW.name IS NOT OLD.name OR
    NEW.description IS NOT OLD.description OR NEW.rank IS NOT OLD.rank OR
    NEW.completed IS NOT OLD.completed OR
    NEW.completed_at IS NOT OLD.completed_at OR NEW.due_date IS NOT OLD.due_date OR
    NEW.state IS NOT OLD.state OR NEW.team_id IS NOT OLD.team_id OR
    NEW.assignee IS NOT OLD.assignee OR
    NEW.completed_at_observed IS NOT OLD.completed_at_observed OR
    NEW.due_date_observed IS NOT OLD.due_date_observed OR
    NEW.assignee_observed IS NOT OLD.assignee_observed OR
    NEW.claimed_project_id IS NOT OLD.claimed_project_id OR
    NEW.project_id_observed IS NOT OLD.project_id_observed OR
    (NEW.observed_at IS NULL) IS NOT (OLD.observed_at IS NULL)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;

DROP TRIGGER store_revision_pm_projects_update;
CREATE TRIGGER store_revision_pm_projects_update AFTER UPDATE ON pm_projects
WHEN NEW.repo IS NOT OLD.repo OR NEW.provider IS NOT OLD.provider OR
    NEW.id IS NOT OLD.id OR NEW.archived IS NOT OLD.archived OR
    NEW.membership_unresolved IS NOT OLD.membership_unresolved OR
    NEW.task_order_json IS NOT OLD.task_order_json OR
    NEW.revision IS NOT OLD.revision OR NEW.slug IS NOT OLD.slug OR
    NEW.name IS NOT OLD.name OR NEW.summary IS NOT OLD.summary OR
    NEW.status IS NOT OLD.status OR NEW.krs IS NOT OLD.krs OR
    NEW.metric_targets IS NOT OLD.metric_targets OR
    NEW.initiative_ids IS NOT OLD.initiative_ids OR
    NEW.team_ids IS NOT OLD.team_ids OR NEW.description IS NOT OLD.description OR
    NEW.description_revision IS NOT OLD.description_revision OR
    (NEW.observed_at IS NULL) IS NOT (OLD.observed_at IS NULL) OR
    (NEW.description_observed_at IS NULL) IS NOT (OLD.description_observed_at IS NULL)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;

DROP TRIGGER store_revision_projects_update;
CREATE TRIGGER store_revision_projects_update AFTER UPDATE ON projects
WHEN NEW.id IS NOT OLD.id OR NEW.wave_id IS NOT OLD.wave_id OR
    NEW.external_project_id IS NOT OLD.external_project_id OR
    NEW.created_at IS NOT OLD.created_at OR
    NEW.project_slug IS NOT OLD.project_slug OR
    NEW.project_name IS NOT OLD.project_name OR
    NEW.project_summary IS NOT OLD.project_summary OR
    NEW.planning_rank IS NOT OLD.planning_rank OR
    NEW.planning_provider_revision IS NOT OLD.planning_provider_revision OR
    NEW.planning_initiatives IS NOT OLD.planning_initiatives OR
    NEW.planning_teams IS NOT OLD.planning_teams OR
    NEW.description IS NOT OLD.description OR
    NEW.updated_at IS NOT OLD.updated_at OR NEW.work_state IS NOT OLD.work_state OR
    NEW.work_terminal_at IS NOT OLD.work_terminal_at OR
    NEW.iteration IS NOT OLD.iteration OR NEW.status IS NOT OLD.status OR
    NEW.legacy_current IS NOT OLD.legacy_current OR NEW.krs IS NOT OLD.krs OR
    NEW.metric_targets IS NOT OLD.metric_targets OR
    (NEW.pm_snapshot_synced_at IS NULL) IS NOT (OLD.pm_snapshot_synced_at IS NULL)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;

DROP TRIGGER store_revision_tasks_update;
CREATE TRIGGER store_revision_tasks_update AFTER UPDATE ON tasks
WHEN NEW.id IS NOT OLD.id OR NEW.repository_id IS NOT OLD.repository_id OR
    NEW.project_id IS NOT OLD.project_id OR
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

DROP TRIGGER store_revision_task_changes_update;
CREATE TRIGGER store_revision_task_changes_update AFTER UPDATE ON task_changes
WHEN NEW.seq IS NOT OLD.seq OR NEW.id IS NOT OLD.id OR
    NEW.task_id IS NOT OLD.task_id OR NEW.value_json IS NOT OLD.value_json OR
    NEW.base_json IS NOT OLD.base_json OR
    NEW.conflict_json IS NOT OLD.conflict_json OR
    NEW.attempted IS NOT OLD.attempted OR NEW.error IS NOT OLD.error OR
    NEW.acknowledged_revision IS NOT OLD.acknowledged_revision OR
    NEW.acknowledged IS NOT OLD.acknowledged
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;

DROP TRIGGER store_revision_task_state_deliveries_update;
CREATE TRIGGER store_revision_task_state_deliveries_update AFTER UPDATE ON task_state_deliveries
WHEN NEW.seq IS NOT OLD.seq OR NEW.id IS NOT OLD.id OR
    NEW.task_id IS NOT OLD.task_id OR NEW.target IS NOT OLD.target OR
    NEW.base_revision IS NOT OLD.base_revision OR
    NEW.base_state IS NOT OLD.base_state OR NEW.attempted IS NOT OLD.attempted OR
    NEW.settled IS NOT OLD.settled OR NEW.error IS NOT OLD.error OR
    NEW.conflict_json IS NOT OLD.conflict_json
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;

