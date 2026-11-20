-- A Wave no longer selects one of its Projects (LOO-995): a new Task joins
-- the newest started Project of its Wave, else of the nearest ancestor Wave
-- that has one, else none, read from `projects` each time. Released
-- lf still reads these members, so they stay, unwritten, until a contract
-- drops them with their idle triggers and rebuilds `waves`.
-- leaving: waves.current_project_id LOO-995
-- leaving: waves.project_activation_lf_process_id LOO-995
-- leaving: waves.project_activation_started_at LOO-995
-- leaving: waves.project_activation_pid LOO-995
-- leaving: waves.project_activation_os_started_at LOO-995
-- leaving: waves.project_activation_ended_at LOO-995
-- leaving: waves.project_activation_exit_code LOO-995
-- leaving: waves.project_activation_error LOO-995
-- Nothing wrote a successor rollover; its row also marked a Project as made
-- on this Machine, which `plan_stamps` now answers.
-- leaving: project_transitions LOO-995
-- KRs and metric targets are no longer sent to a tracker; an unsent edit of
-- either is retired, and their columns stay unwritten until the contract.
UPDATE tracker_actions SET phase='retired',error='KRs and metric targets are no longer sent'
WHERE subject IN ('krs','metric_targets') AND phase IN ('queued','sent','conflicted');
-- leaving: projects.krs LOO-995
-- leaving: projects.metric_targets LOO-995
-- leaving: tracker_projects.krs LOO-995
-- leaving: tracker_projects.metric_targets LOO-995
-- leaving: pm_projects.krs LOO-995
-- leaving: pm_projects.metric_targets LOO-995
-- Nothing reads or writes these since chapters ended.
-- leaving: projects.legacy_current LOO-995
-- leaving: project_events LOO-995
-- leaving: projects.iteration LOO-995
-- Live metrics are gone: no contract, instrument or observation is read.
-- leaving: metric_instruments LOO-995
-- leaving: metric_observations LOO-995
-- A Task records its own Wave. Its Project is optional: the Wave's, else the
-- nearest ancestor Wave's, else none, so the Wave is no longer read through
-- the Project.
ALTER TABLE tasks ADD COLUMN wave_id TEXT REFERENCES waves(id);
UPDATE tasks SET wave_id=(SELECT p.wave_id FROM projects p WHERE p.id=tasks.project_id)
WHERE project_id IS NOT NULL;
CREATE INDEX tasks_wave ON tasks(wave_id);
DROP TRIGGER store_revision_tasks_update;
CREATE TRIGGER store_revision_tasks_update AFTER UPDATE ON tasks
WHEN NEW.id IS NOT OLD.id OR NEW.repository_id IS NOT OLD.repository_id OR
    NEW.project_id IS NOT OLD.project_id OR NEW.wave_id IS NOT OLD.wave_id OR
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
