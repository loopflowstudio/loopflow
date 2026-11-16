-- LOO-935 drops what the product no longer reads or writes, without the
-- usual staging wait: a one-time exception `check_migrations.py` names for
-- this file alone. lf releases that still read these members fail on the
-- statement that names one; the rest of the store still answers.
-- exports: <lf home>/exports/unused-store-members.json

-- Still read: a later Task drops each once its last read goes.
-- leaving: agent_sessions.work_source LOO-836
-- Released peers still exchange it through planning.
-- leaving: tasks.regressed_in LOO-730
-- leaving: provider_accounts LOO-928
-- leaving: provider_session_accounts LOO-928
-- leaving: provider_account_limits LOO-928
-- leaving: provider_account_switches LOO-928

-- Never had a caller, finished one-time migrations and imports, write-only
-- records, and screen state Desktop now keeps in its own files.
DROP TABLE tool_responses;
DROP TABLE pm_project_name_cutover;
DROP TABLE project_transition_items;
DROP TABLE task_issue_identities;
DROP TABLE task_deletions;
DROP TABLE task_pr_repair_incidents;
DROP TABLE project_binding_imports;
DROP TABLE work_pane_layouts;
-- Steering moved to `steers` (#1923).
DROP TABLE session_steers;
DROP TABLE steer_deliveries;
DROP TABLE steer_batches;

-- Pool and on-demand declarations live in `config.yaml`; these held only a
-- name and a creation time. An owned Machine's pool is that name.
DROP TABLE pools;
DROP TABLE ondemands;
CREATE TABLE pool_machines_rebuilt (
    machine_id TEXT PRIMARY KEY REFERENCES machines(id) ON DELETE CASCADE,
    pool TEXT,
    cost_usd TEXT,
    expires_at INTEGER
);
INSERT INTO pool_machines_rebuilt(machine_id,pool,cost_usd,expires_at)
SELECT machine_id,pool,cost_usd,expires_at FROM pool_machines;
DROP TABLE pool_machines;
ALTER TABLE pool_machines_rebuilt RENAME TO pool_machines;
CREATE INDEX pool_machines_pool ON pool_machines(pool);
ALTER TABLE machines DROP COLUMN pool;
ALTER TABLE machines DROP COLUMN cost_usd;
ALTER TABLE machines DROP COLUMN expires_at;

-- `held_terminals` holds the live viewer state; Sessions take input natively.
ALTER TABLE processes DROP COLUMN viewer_hold;
ALTER TABLE processes DROP COLUMN viewer_hold_at;
ALTER TABLE processes DROP COLUMN viewed;
ALTER TABLE processes DROP COLUMN native_input;
-- A Close ends a held shell directly; no owner polls for it.
ALTER TABLE desktop_shells DROP COLUMN close_requested;

-- Only the dropped repair incidents read whether repair tracking was complete.
DROP TRIGGER task_prs_enable_performance_tracking;
ALTER TABLE task_prs DROP COLUMN repair_tracking_complete;
CREATE TRIGGER task_prs_enable_performance_tracking
AFTER INSERT ON task_prs
BEGIN
    UPDATE task_prs SET merge_tracking_complete = 1 WHERE id = NEW.id;
END;

-- `task_ownership.writer_machine_id` replaced `written_here`.
DROP TRIGGER task_ownership_append_only;
ALTER TABLE task_ownership DROP COLUMN written_here;
CREATE TRIGGER task_ownership_append_only BEFORE UPDATE ON task_ownership
WHEN NOT (OLD.ordered_at IS NULL AND NEW.ordered_at IS NOT NULL
    AND NEW.id IS OLD.id AND NEW.task_id IS OLD.task_id AND NEW.kind IS OLD.kind
    AND NEW.person IS OLD.person AND NEW.machine_id IS OLD.machine_id
    AND NEW.at IS OLD.at
    AND NEW.person_key IS OLD.person_key AND NEW.machine_label IS OLD.machine_label
    AND NEW.writer_machine_id IS OLD.writer_machine_id AND NEW.person_id IS OLD.person_id)
BEGIN SELECT RAISE(ABORT, 'Task ownership entries are append-only'); END;

-- `task_checkouts`, `task_primary_sessions` and `task_ship_events` replaced
-- these Task columns (`task_machine_local`).
DROP TRIGGER task_placement_to_pr;
DROP TRIGGER canonical_tasks_insert;
DROP TRIGGER canonical_tasks_update;
DROP TRIGGER task_update_tasks_update;
DROP TRIGGER cancel_task_retirement;
DROP TRIGGER store_revision_tasks_update;
DROP INDEX idx_tasks_worktree;
DROP INDEX tasks_branch;
DROP INDEX tasks_ship_process;
DROP INDEX tasks_checkout_root;
DROP INDEX tasks_primary_session;
DROP INDEX tasks_abandoned_checkout;
DROP INDEX tasks_checkout_machine;
DROP INDEX tasks_parent_pr;
ALTER TABLE tasks DROP COLUMN worktree;
ALTER TABLE tasks DROP COLUMN branch;
ALTER TABLE tasks DROP COLUMN base_commit;
ALTER TABLE tasks DROP COLUMN parent_pr_id;
ALTER TABLE tasks DROP COLUMN checkout_machine_id;
ALTER TABLE tasks DROP COLUMN primary_session_id;
ALTER TABLE tasks DROP COLUMN ship_lf_process_id;
ALTER TABLE tasks DROP COLUMN ship_restarts;
ALTER TABLE tasks DROP COLUMN ship_approved_lf_process_id;
ALTER TABLE tasks DROP COLUMN ship_stopped_at;
ALTER TABLE tasks DROP COLUMN ship_stopped_lf_process_id;
ALTER TABLE tasks DROP COLUMN ship_stopped_person;
CREATE TRIGGER canonical_tasks_insert BEFORE INSERT ON tasks
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tasks') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.repository_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.external_issue_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_assignee AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_team_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.issue_identifier AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

CREATE TRIGGER canonical_tasks_update BEFORE UPDATE OF external_issue_id,planning_assignee,planning_team_id,id,project_id,issue_identifier,repository_id ON tasks
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tasks') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.repository_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.external_issue_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_assignee AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_team_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.issue_identifier AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

CREATE TRIGGER store_revision_tasks_update AFTER UPDATE ON tasks
WHEN NEW.id IS NOT OLD.id OR NEW.repository_id IS NOT OLD.repository_id OR
    NEW.project_id IS NOT OLD.project_id OR
    NEW.external_issue_id IS NOT OLD.external_issue_id OR
    NEW.issue_identifier IS NOT OLD.issue_identifier OR
    NEW.created_at IS NOT OLD.created_at OR
    NEW.issue_title IS NOT OLD.issue_title OR
    NEW.issue_description IS NOT OLD.issue_description OR
    NEW.workspace_slug IS NOT OLD.workspace_slug OR
    NEW.abandon_reason IS NOT OLD.abandon_reason OR
    NEW.updated_at IS NOT OLD.updated_at OR
    NEW.abandoned_at IS NOT OLD.abandoned_at OR
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
    NEW.completion_request IS NOT OLD.completion_request OR
    NEW.completion_error IS NOT OLD.completion_error OR
    NEW.planning_due_date IS NOT OLD.planning_due_date OR
    NEW.paused IS NOT OLD.paused OR NEW.shipped_at IS NOT OLD.shipped_at OR
    NEW.regressed_in IS NOT OLD.regressed_in OR
    (NEW.pm_snapshot_synced_at IS NULL) IS NOT (OLD.pm_snapshot_synced_at IS NULL)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;

CREATE TRIGGER task_update_tasks_update AFTER UPDATE OF issue_identifier ON tasks
WHEN NEW.issue_identifier IS NOT OLD.issue_identifier
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER cancel_task_retirement
AFTER UPDATE OF planning_completed,planning_completed_at,abandoned_at ON tasks
WHEN (NEW.planning_completed=0 AND NEW.abandoned_at IS NULL)
  OR NEW.planning_completed_at IS NOT OLD.planning_completed_at
  OR NEW.abandoned_at IS NOT OLD.abandoned_at
BEGIN
    DELETE FROM task_retirements WHERE task_id=NEW.id;
END;

-- Dashboards are screen state: an edit no longer wakes planning readers.
DROP TRIGGER store_revision_dashboards_insert;
DROP TRIGGER store_revision_dashboards_update;
DROP TRIGGER store_revision_dashboards_delete;
DROP TRIGGER store_revision_dashboard_arrangements_insert;
DROP TRIGGER store_revision_dashboard_arrangements_update;
DROP TRIGGER store_revision_dashboard_arrangements_delete;
