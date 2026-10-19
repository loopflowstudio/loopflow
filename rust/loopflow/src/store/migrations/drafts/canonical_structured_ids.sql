-- depends_on: none
-- A Trace owns one journal directory. Case aliases may have distinct directories
-- even when no Process primary keys collide. Never choose one and lose the other.
CREATE TEMP TABLE canonical_identity_conflicts (domain TEXT, first TEXT, second TEXT);
CREATE TEMP TRIGGER canonical_identity_conflict BEFORE INSERT ON canonical_identity_conflicts
BEGIN
    SELECT RAISE(ABORT, 'canonical identity collision: ' || NEW.domain || ': '
        || NEW.first || ' / ' || NEW.second);
END;
INSERT INTO canonical_identity_conflicts
SELECT 'Trace journal locator',min(trace_id),max(trace_id)
FROM processes GROUP BY lower(trace_id) HAVING count(DISTINCT trace_id)>1;
DROP TRIGGER canonical_identity_conflict;
DROP TABLE canonical_identity_conflicts;

-- Filesystem addresses are opaque locators on their existing owners. Retain the
-- exact spelling; no file is renamed and no waiting caller changes lock inode.
ALTER TABLE agent_sessions ADD COLUMN fence_locator TEXT;
UPDATE agent_sessions SET fence_locator=id;
CREATE TRIGGER session_fence_locator_insert AFTER INSERT ON agent_sessions
WHEN NEW.fence_locator IS NULL
BEGIN
    UPDATE agent_sessions SET fence_locator=NEW.id WHERE id=NEW.id;
END;
CREATE TRIGGER session_fence_locator_immutable BEFORE UPDATE OF fence_locator ON agent_sessions
WHEN OLD.fence_locator IS NOT NULL AND NEW.fence_locator IS NOT OLD.fence_locator
BEGIN
    SELECT RAISE(ABORT, 'Session fence locator is immutable');
END;

ALTER TABLE processes ADD COLUMN receipt_locator TEXT;
ALTER TABLE processes ADD COLUMN journal_trace_locator TEXT;
UPDATE processes SET receipt_locator=id,journal_trace_locator=trace_id;
CREATE UNIQUE INDEX processes_receipt_locator ON processes(receipt_locator);
CREATE TRIGGER process_locators_insert AFTER INSERT ON processes
BEGIN
    UPDATE processes SET receipt_locator=COALESCE(NEW.receipt_locator,NEW.id),
        journal_trace_locator=COALESCE(NEW.journal_trace_locator,
            (SELECT journal_trace_locator FROM processes WHERE trace_id=NEW.trace_id
             AND id!=NEW.id ORDER BY started_at,id LIMIT 1),NEW.trace_id)
    WHERE id=NEW.id;
END;
CREATE TRIGGER process_locators_immutable BEFORE UPDATE OF receipt_locator,journal_trace_locator ON processes
WHEN (OLD.receipt_locator IS NOT NULL AND NEW.receipt_locator IS NOT OLD.receipt_locator)
    OR (OLD.journal_trace_locator IS NOT NULL AND NEW.journal_trace_locator IS NOT OLD.journal_trace_locator)
BEGIN
    SELECT RAISE(ABORT, 'Process filesystem locators are immutable');
END;

-- A stale waiter may still hold the original inode after commit. It must re-read
-- attachment authority, and cannot recreate an owner using its old spelling.
-- Released Session identities are bare hex or session_<hex>; other historical
-- Session names remain opaque.
CREATE TRIGGER canonical_session_id_insert BEFORE INSERT ON agent_sessions
WHEN NEW.id != lower(NEW.id) AND (
    (length(NEW.id)=32 AND NEW.id NOT GLOB '*[^0-9a-fA-F]*') OR
    (length(NEW.id)=40 AND lower(substr(NEW.id,1,8))='session_'
        AND substr(NEW.id,9) NOT GLOB '*[^0-9a-fA-F]*'))
BEGIN
    SELECT RAISE(ABORT, 'Session identity must be canonical');
END;
CREATE TRIGGER canonical_session_id_update BEFORE UPDATE OF id ON agent_sessions
WHEN NEW.id != lower(NEW.id) AND (
    (length(NEW.id)=32 AND NEW.id NOT GLOB '*[^0-9a-fA-F]*') OR
    (length(NEW.id)=40 AND lower(substr(NEW.id,1,8))='session_'
        AND substr(NEW.id,9) NOT GLOB '*[^0-9a-fA-F]*'))
BEGIN
    SELECT RAISE(ABORT, 'Session identity must be canonical');
END;






-- Convert only domain-owned values. Temporary copies permit collision checks before touching owners.

CREATE TEMP TABLE canonical_agent_sessions AS SELECT rowid AS source_rowid,* FROM agent_sessions;

CREATE TEMP TABLE canonical_ci_incidents AS SELECT rowid AS source_rowid,* FROM ci_incidents;

CREATE TEMP TABLE canonical_flow_process_steps AS SELECT rowid AS source_rowid,* FROM flow_process_steps;

CREATE TEMP TABLE canonical_flow_processes AS SELECT rowid AS source_rowid,* FROM flow_processes;

CREATE TEMP TABLE canonical_linear_team_checks AS SELECT rowid AS source_rowid,* FROM linear_team_checks;

CREATE TEMP TABLE canonical_machines AS SELECT rowid AS source_rowid,* FROM machines;

CREATE TEMP TABLE canonical_metric_instruments AS SELECT rowid AS source_rowid,* FROM metric_instruments;

CREATE TEMP TABLE canonical_metric_observations AS SELECT rowid AS source_rowid,* FROM metric_observations;

CREATE TEMP TABLE canonical_planning_peer_changes AS SELECT rowid AS source_rowid,* FROM planning_peer_changes;

CREATE TEMP TABLE canonical_planning_peer_conflicts AS SELECT rowid AS source_rowid,* FROM planning_peer_conflicts;

CREATE TEMP TABLE canonical_planning_peer_observed AS SELECT rowid AS source_rowid,* FROM planning_peer_observed;

CREATE TEMP TABLE canonical_planning_peer_routes AS SELECT rowid AS source_rowid,* FROM planning_peer_routes;

CREATE TEMP TABLE canonical_pm_issue_changes AS SELECT rowid AS source_rowid,* FROM pm_issue_changes;

CREATE TEMP TABLE canonical_pm_items AS SELECT rowid AS source_rowid,* FROM pm_items;

CREATE TEMP TABLE canonical_pm_project_name_cutover AS SELECT rowid AS source_rowid,* FROM pm_project_name_cutover;

CREATE TEMP TABLE canonical_pm_projects AS SELECT rowid AS source_rowid,* FROM pm_projects;

CREATE TEMP TABLE canonical_pm_wave_projects AS SELECT rowid AS source_rowid,* FROM pm_wave_projects;

CREATE TEMP TABLE canonical_pm_wave_sync AS SELECT rowid AS source_rowid,* FROM pm_wave_sync;

CREATE TEMP TABLE canonical_pr_landings AS SELECT rowid AS source_rowid,* FROM pr_landings;

CREATE TEMP TABLE canonical_processes AS SELECT rowid AS source_rowid,* FROM processes;

CREATE TEMP TABLE canonical_project_binding_imports AS SELECT rowid AS source_rowid,* FROM project_binding_imports;

CREATE TEMP TABLE canonical_project_changes AS SELECT rowid AS source_rowid,* FROM project_changes;

CREATE TEMP TABLE canonical_project_events AS SELECT rowid AS source_rowid,* FROM project_events;

CREATE TEMP TABLE canonical_project_transition_items AS SELECT rowid AS source_rowid,* FROM project_transition_items;

CREATE TEMP TABLE canonical_project_transitions AS SELECT rowid AS source_rowid,* FROM project_transitions;

CREATE TEMP TABLE canonical_projects AS SELECT rowid AS source_rowid,* FROM projects;

CREATE TEMP TABLE canonical_relocated_task_processes AS SELECT rowid AS source_rowid,* FROM relocated_task_processes;

CREATE TEMP TABLE canonical_repository_plans AS SELECT rowid AS source_rowid,* FROM repository_plans;

CREATE TEMP TABLE canonical_session_activity AS SELECT rowid AS source_rowid,* FROM session_activity;

CREATE TEMP TABLE canonical_session_events AS SELECT rowid AS source_rowid,* FROM session_events;

CREATE TEMP TABLE canonical_session_steers AS SELECT rowid AS source_rowid,* FROM session_steers;

CREATE TEMP TABLE canonical_steer_adapters AS SELECT rowid AS source_rowid,* FROM steer_adapters;

CREATE TEMP TABLE canonical_steer_batches AS SELECT rowid AS source_rowid,* FROM steer_batches;

CREATE TEMP TABLE canonical_steer_deliveries AS SELECT rowid AS source_rowid,* FROM steer_deliveries;

CREATE TEMP TABLE canonical_steers AS SELECT rowid AS source_rowid,* FROM steers;

CREATE TEMP TABLE canonical_task_admissions AS SELECT rowid AS source_rowid,* FROM task_admissions;

CREATE TEMP TABLE canonical_task_changes AS SELECT rowid AS source_rowid,* FROM task_changes;

CREATE TEMP TABLE canonical_task_comment_deliveries AS SELECT rowid AS source_rowid,* FROM task_comment_deliveries;

CREATE TEMP TABLE canonical_task_comments AS SELECT rowid AS source_rowid,* FROM task_comments;

CREATE TEMP TABLE canonical_task_creation_intents AS SELECT rowid AS source_rowid,* FROM task_creation_intents;

CREATE TEMP TABLE canonical_task_deletions AS SELECT rowid AS source_rowid,* FROM task_deletions;

CREATE TEMP TABLE canonical_task_events AS SELECT rowid AS source_rowid,* FROM task_events;

CREATE TEMP TABLE canonical_task_issue_identities AS SELECT rowid AS source_rowid,* FROM task_issue_identities;

CREATE TEMP TABLE canonical_task_linear_ingested_comments AS SELECT rowid AS source_rowid,* FROM task_linear_ingested_comments;

CREATE TEMP TABLE canonical_task_linear_observations AS SELECT rowid AS source_rowid,* FROM task_linear_observations;

CREATE TEMP TABLE canonical_task_pr_repair_incidents AS SELECT rowid AS source_rowid,* FROM task_pr_repair_incidents;

CREATE TEMP TABLE canonical_task_prs AS SELECT rowid AS source_rowid,* FROM task_prs;

CREATE TEMP TABLE canonical_task_state_deliveries AS SELECT rowid AS source_rowid,* FROM task_state_deliveries;

CREATE TEMP TABLE canonical_task_workflow_moves AS SELECT rowid AS source_rowid,* FROM task_workflow_moves;

CREATE TEMP TABLE canonical_task_workflows AS SELECT rowid AS source_rowid,* FROM task_workflows;

CREATE TEMP TABLE canonical_tasks AS SELECT rowid AS source_rowid,* FROM tasks;

CREATE TEMP TABLE canonical_tool_responses AS SELECT rowid AS source_rowid,* FROM tool_responses;

CREATE TEMP TABLE canonical_wave_workflows AS SELECT rowid AS source_rowid,* FROM wave_workflows;

CREATE TEMP TABLE canonical_waves AS SELECT rowid AS source_rowid,* FROM waves;

CREATE TEMP TABLE canonical_work_placements AS SELECT rowid AS source_rowid,* FROM work_placements;

CREATE TEMP TABLE canonical_values(domain TEXT NOT NULL,original TEXT NOT NULL,canonical TEXT,PRIMARY KEY(domain,original));

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',id FROM waves WHERE id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',parent_wave_id FROM waves WHERE parent_wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',superseded_by_wave_id FROM waves WHERE superseded_by_wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',project_activation_lf_process_id FROM waves WHERE project_activation_lf_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'proj_',current_project_id FROM waves WHERE current_project_id IS NOT NULL AND 'proj_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',id FROM processes WHERE id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',trace_id FROM processes WHERE trace_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',parent_lf_process_id FROM processes WHERE parent_lf_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',caller_agent_process_id FROM processes WHERE caller_agent_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',attached_lf_process_id FROM processes WHERE attached_lf_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',attachment_token FROM processes WHERE attachment_token IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'session',caller_session_id FROM processes WHERE caller_session_id IS NOT NULL AND 'session' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'session',agent_session_id FROM processes WHERE agent_session_id IS NOT NULL AND 'session' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',wave_id FROM agent_sessions WHERE wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',agent_process_id FROM agent_sessions WHERE agent_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',transport_lf_process_id FROM agent_sessions WHERE transport_lf_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM agent_sessions WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'session',id FROM agent_sessions WHERE id IS NOT NULL AND 'session' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',lf_process_id FROM session_events WHERE lf_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',wave_id FROM session_events WHERE wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',agent_process_id FROM session_events WHERE agent_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM session_events WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'session',session_id FROM session_events WHERE session_id IS NOT NULL AND 'session' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',attachment_token FROM session_activity WHERE attachment_token IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',agent_process_id FROM session_activity WHERE agent_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'session',session_id FROM session_activity WHERE session_id IS NOT NULL AND 'session' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',lf_process_id FROM flow_processes WHERE lf_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',lf_process_id FROM flow_process_steps WHERE lf_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',flow_lf_process_id FROM flow_process_steps WHERE flow_lf_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',lf_process_id FROM task_workflows WHERE lf_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM task_workflows WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',lf_process_id FROM task_workflow_moves WHERE lf_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM task_workflow_moves WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',wave_id FROM projects WHERE wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',external_project_id FROM projects WHERE external_project_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'proj_',id FROM projects WHERE id IS NOT NULL AND 'proj_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',external_issue_id FROM tasks WHERE external_issue_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',planning_assignee FROM tasks WHERE planning_assignee IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',planning_team_id FROM tasks WHERE planning_team_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',id FROM tasks WHERE id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'proj_',project_id FROM tasks WHERE project_id IS NOT NULL AND 'proj_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'home_',checkout_machine_id FROM tasks WHERE checkout_machine_id IS NOT NULL AND 'home_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'pr_',parent_pr_id FROM tasks WHERE parent_pr_id IS NOT NULL AND 'pr_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'session',primary_session_id FROM tasks WHERE primary_session_id IS NOT NULL AND 'session' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'label',issue_identifier FROM tasks WHERE issue_identifier IS NOT NULL AND 'label' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',linear_attachment_id FROM task_prs WHERE linear_attachment_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',linear_comment_id FROM task_prs WHERE linear_comment_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM task_prs WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'pr_',id FROM task_prs WHERE id IS NOT NULL AND 'pr_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'pr_',parent_pr_id FROM task_prs WHERE parent_pr_id IS NOT NULL AND 'pr_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',repair_lf_process_id FROM ci_incidents WHERE repair_lf_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM ci_incidents WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'pr_',pr_id FROM ci_incidents WHERE pr_id IS NOT NULL AND 'pr_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'landing_',landing_id FROM ci_incidents WHERE landing_id IS NOT NULL AND 'landing_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'session',repair_session_id FROM ci_incidents WHERE repair_session_id IS NOT NULL AND 'session' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',comment_id FROM task_linear_ingested_comments WHERE comment_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM task_linear_ingested_comments WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',wave_id FROM task_deletions WHERE wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',issue_id FROM task_deletions WHERE issue_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'label',identifier FROM task_deletions WHERE identifier IS NOT NULL AND 'label' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',wave_id FROM task_issue_identities WHERE wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',issue_id FROM task_issue_identities WHERE issue_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'label',identifier FROM task_issue_identities WHERE identifier IS NOT NULL AND 'label' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',id FROM pm_projects WHERE id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',id FROM pm_items WHERE id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',project_id FROM pm_items WHERE project_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',team_id FROM pm_items WHERE team_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',assignee FROM pm_items WHERE assignee IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',claimed_project_id FROM pm_items WHERE claimed_project_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'label',identifier FROM pm_items WHERE identifier IS NOT NULL AND 'label' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',wave_id FROM pm_wave_sync WHERE wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',initiative FROM pm_wave_sync WHERE initiative IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',wave_id FROM pm_wave_projects WHERE wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',project_id FROM pm_wave_projects WHERE project_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',issue_id FROM pm_issue_changes WHERE issue_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',id FROM pm_project_name_cutover WHERE id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',wave_id FROM project_binding_imports WHERE wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',wave_id FROM wave_workflows WHERE wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',wave_id FROM project_transitions WHERE wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'project_or_uuid',successor_id FROM project_transitions WHERE successor_id IS NOT NULL AND 'project_or_uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'project_or_uuid',predecessor_id FROM project_transitions WHERE predecessor_id IS NOT NULL AND 'project_or_uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',wave_id FROM project_transition_items WHERE wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'project_or_uuid',successor_id FROM project_transition_items WHERE successor_id IS NOT NULL AND 'project_or_uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_or_uuid',issue_id FROM project_transition_items WHERE issue_id IS NOT NULL AND 'task_or_uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',id FROM task_state_deliveries WHERE id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM task_state_deliveries WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',id FROM task_comments WHERE id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM task_comments WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',comment_id FROM task_comment_deliveries WHERE comment_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',id FROM project_changes WHERE id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'proj_',project_id FROM project_changes WHERE project_id IS NOT NULL AND 'proj_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',id FROM task_changes WHERE id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM task_changes WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',id FROM planning_peer_changes WHERE id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',id FROM planning_peer_observed WHERE id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',mutation_id FROM planning_peer_routes WHERE mutation_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',agent_process_id FROM steer_adapters WHERE agent_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',id FROM steers WHERE id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',wave_id FROM steers WHERE wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM steers WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'proj_',project_id FROM steers WHERE project_id IS NOT NULL AND 'proj_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'session',session_id FROM steers WHERE session_id IS NOT NULL AND 'session' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',steer_id FROM session_steers WHERE steer_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'session',session_id FROM session_steers WHERE session_id IS NOT NULL AND 'session' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',steer_id FROM steer_deliveries WHERE steer_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',batch_id FROM steer_deliveries WHERE batch_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'session',session_id FROM steer_deliveries WHERE session_id IS NOT NULL AND 'session' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',id FROM steer_batches WHERE id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',agent_process_id FROM steer_batches WHERE agent_process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',attachment_token FROM steer_batches WHERE attachment_token IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'session',session_id FROM steer_batches WHERE session_id IS NOT NULL AND 'session' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',wave_id FROM work_placements WHERE wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM work_placements WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'proj_',project_id FROM work_placements WHERE project_id IS NOT NULL AND 'proj_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'home_',machine_id FROM work_placements WHERE machine_id IS NOT NULL AND 'home_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',wave_id FROM metric_instruments WHERE wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',wave_id FROM metric_observations WHERE wave_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',team_id FROM linear_team_checks WHERE team_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'uuid',process_id FROM relocated_task_processes WHERE process_id IS NOT NULL AND 'uuid' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM relocated_task_processes WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM task_events WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM pr_landings WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'home_',supervisor_machine_id FROM pr_landings WHERE supervisor_machine_id IS NOT NULL AND 'home_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'landing_',id FROM pr_landings WHERE id IS NOT NULL AND 'landing_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM task_linear_observations WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM task_creation_intents WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'proj_',project_id FROM task_creation_intents WHERE project_id IS NOT NULL AND 'proj_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'task_',task_id FROM task_admissions WHERE task_id IS NOT NULL AND 'task_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'home_',machine_id FROM task_admissions WHERE machine_id IS NOT NULL AND 'home_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'proj_',project_id FROM project_events WHERE project_id IS NOT NULL AND 'proj_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'home_',id FROM machines WHERE id IS NOT NULL AND 'home_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'pr_',task_pr_id FROM task_pr_repair_incidents WHERE task_pr_id IS NOT NULL AND 'pr_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'repo_',id FROM repository_plans WHERE id IS NOT NULL AND 'repo_' IS NOT NULL;
INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'repo_',repository_id FROM tasks WHERE repository_id IS NOT NULL AND 'repo_' IS NOT NULL;
INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'repo_',repository_id FROM task_creation_intents WHERE repository_id IS NOT NULL AND 'repo_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT 'response_',id FROM tool_responses WHERE id IS NOT NULL AND 'response_' IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT CASE work_kind WHEN 'wave' THEN 'uuid' WHEN 'project' THEN 'proj_' WHEN 'task' THEN 'task_' END,work_id FROM tool_responses WHERE work_id IS NOT NULL AND CASE work_kind WHEN 'wave' THEN 'uuid' WHEN 'project' THEN 'proj_' WHEN 'task' THEN 'task_' END IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT CASE kind WHEN 'waves' THEN 'uuid' WHEN 'projects' THEN 'proj_' WHEN 'tasks' THEN 'task_' WHEN 'task_comments' THEN 'uuid' END,object_id FROM planning_peer_conflicts WHERE object_id IS NOT NULL AND CASE kind WHEN 'waves' THEN 'uuid' WHEN 'projects' THEN 'proj_' WHEN 'tasks' THEN 'task_' WHEN 'task_comments' THEN 'uuid' END IS NOT NULL;

CREATE TEMP TABLE canonical_json(owner TEXT,column_name TEXT,source_rowid INTEGER,path TEXT,parent TEXT,domain TEXT,original TEXT,PRIMARY KEY(owner,column_name,source_rowid,path));

INSERT INTO canonical_json
SELECT 'tasks','admission_authority',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.admission_authority AS doc, CASE WHEN path='$.machine_id' THEN 'home_' WHEN path='$.repository_id' THEN 'repo_' END AS domain
 FROM canonical_tasks owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.admission_authority AS doc FROM canonical_tasks source,json_tree(CASE WHEN json_valid(source.admission_authority) THEN source.admission_authority END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'projects','planning_initiatives',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.planning_initiatives AS doc, CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain
 FROM canonical_projects owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.planning_initiatives AS doc FROM canonical_projects source,json_tree(CASE WHEN json_valid(source.planning_initiatives) THEN source.planning_initiatives END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'projects','planning_teams',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.planning_teams AS doc, CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain
 FROM canonical_projects owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.planning_teams AS doc FROM canonical_projects source,json_tree(CASE WHEN json_valid(source.planning_teams) THEN source.planning_teams END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'pm_projects','initiative_ids',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.initiative_ids AS doc, CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain
 FROM canonical_pm_projects owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.initiative_ids AS doc FROM canonical_pm_projects source,json_tree(CASE WHEN json_valid(source.initiative_ids) THEN source.initiative_ids END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'pm_projects','team_ids',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.team_ids AS doc, CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain
 FROM canonical_pm_projects owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.team_ids AS doc FROM canonical_pm_projects source,json_tree(CASE WHEN json_valid(source.team_ids) THEN source.team_ids END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'pm_projects','task_order_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.task_order_json AS doc, CASE WHEN parent='$' AND typeof(key)='integer' THEN 'task_' END AS domain
 FROM canonical_pm_projects owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.task_order_json AS doc FROM canonical_pm_projects source,json_tree(CASE WHEN json_valid(source.task_order_json) THEN source.task_order_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'task_comment_deliveries','comment_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.comment_json AS doc, CASE WHEN path='$.id' THEN 'uuid' END AS domain
 FROM canonical_task_comment_deliveries owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.comment_json AS doc FROM canonical_task_comment_deliveries source,json_tree(CASE WHEN json_valid(source.comment_json) THEN source.comment_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'task_comment_deliveries','conflicting_comment_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.conflicting_comment_json AS doc, CASE WHEN path='$.id' THEN 'uuid' END AS domain
 FROM canonical_task_comment_deliveries owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.conflicting_comment_json AS doc FROM canonical_task_comment_deliveries source,json_tree(CASE WHEN json_valid(source.conflicting_comment_json) THEN source.conflicting_comment_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'project_transitions','local_plan_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.local_plan_json AS doc, CASE WHEN path='$.wave_id' THEN 'uuid' WHEN path='$.successor_id' THEN 'project_or_uuid' END AS domain
 FROM canonical_project_transitions owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.local_plan_json AS doc FROM canonical_project_transitions source,json_tree(CASE WHEN json_valid(source.local_plan_json) THEN source.local_plan_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'project_transitions','export_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.export_json AS doc, CASE WHEN path='$.id' THEN 'uuid' WHEN path='$.initiative' THEN 'uuid' WHEN path='$.link_id' THEN 'uuid' WHEN path='$.input.id' THEN 'uuid' WHEN path='$.input.teamId' THEN 'uuid' WHEN path='$.input.projectId' THEN 'uuid' WHEN path='$.input.assigneeId' THEN 'uuid' WHEN parent='$.input.teamIds' AND typeof(key)='integer' THEN 'uuid' WHEN (parent GLOB '$.baseline[[]*].value') AND typeof(key)='integer' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*].value' AND key='machine_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN parent GLOB '$.baseline[[]*].value' AND key='repository_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field')='task_admission_authority' THEN 'repo_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_project','task_project') THEN 'proj_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('comment_task') THEN 'task_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('task_identifier') THEN 'label' END AS domain
 FROM canonical_project_transitions owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.export_json AS doc FROM canonical_project_transitions source,json_tree(CASE WHEN json_valid(source.export_json) THEN source.export_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'task_creation_intents','export_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.export_json AS doc, CASE WHEN path='$.id' THEN 'uuid' WHEN path='$.initiative' THEN 'uuid' WHEN path='$.link_id' THEN 'uuid' WHEN path='$.input.id' THEN 'uuid' WHEN path='$.input.teamId' THEN 'uuid' WHEN path='$.input.projectId' THEN 'uuid' WHEN path='$.input.assigneeId' THEN 'uuid' WHEN parent='$.input.teamIds' AND typeof(key)='integer' THEN 'uuid' WHEN (parent GLOB '$.baseline[[]*].value') AND typeof(key)='integer' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*].value' AND key='machine_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN parent GLOB '$.baseline[[]*].value' AND key='repository_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field')='task_admission_authority' THEN 'repo_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_project','task_project') THEN 'proj_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('comment_task') THEN 'task_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('task_identifier') THEN 'label' END AS domain
 FROM canonical_task_creation_intents owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.export_json AS doc FROM canonical_task_creation_intents source,json_tree(CASE WHEN json_valid(source.export_json) THEN source.export_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'task_changes','value_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.value_json AS doc, CASE WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('comment_task') THEN 'task_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value' AND typeof(key)='integer' AND json_extract(doc,'$.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.machine_id' AND json_extract(doc,'$.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.repository_id' AND json_extract(doc,'$.field')='task_admission_authority' THEN 'repo_' END AS domain
 FROM canonical_task_changes owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.value_json AS doc FROM canonical_task_changes source,json_tree(CASE WHEN json_valid(source.value_json) THEN source.value_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'task_changes','base_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.base_json AS doc, CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' END AS domain
 FROM canonical_task_changes owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.base_json AS doc FROM canonical_task_changes source,json_tree(CASE WHEN json_valid(source.base_json) THEN source.base_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'task_changes','conflict_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.conflict_json AS doc, CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' END AS domain
 FROM canonical_task_changes owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.conflict_json AS doc FROM canonical_task_changes source,json_tree(CASE WHEN json_valid(source.conflict_json) THEN source.conflict_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'project_changes','value_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.value_json AS doc, CASE WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('comment_task') THEN 'task_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value' AND typeof(key)='integer' AND json_extract(doc,'$.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.machine_id' AND json_extract(doc,'$.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.repository_id' AND json_extract(doc,'$.field')='task_admission_authority' THEN 'repo_' WHEN parent='$' AND typeof(key)='integer' THEN 'task_' END AS domain
 FROM canonical_project_changes owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.value_json AS doc FROM canonical_project_changes source,json_tree(CASE WHEN json_valid(source.value_json) THEN source.value_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'project_changes','base_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.base_json AS doc, CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' WHEN parent='$.value' AND typeof(key)='integer' THEN 'task_' END AS domain
 FROM canonical_project_changes owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.base_json AS doc FROM canonical_project_changes source,json_tree(CASE WHEN json_valid(source.base_json) THEN source.base_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'project_changes','conflict_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.conflict_json AS doc, CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' WHEN parent='$.value' AND typeof(key)='integer' THEN 'task_' END AS domain
 FROM canonical_project_changes owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.conflict_json AS doc FROM canonical_project_changes source,json_tree(CASE WHEN json_valid(source.conflict_json) THEN source.conflict_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'project_changes','order_effects_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.order_effects_json AS doc, CASE WHEN (parent GLOB '$[[]*].before' OR parent GLOB '$[[]*].after') AND typeof(key)='integer' THEN 'task_' WHEN parent GLOB '$[[]*]' AND key='issue' THEN 'uuid' END AS domain
 FROM canonical_project_changes owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.order_effects_json AS doc FROM canonical_project_changes source,json_tree(CASE WHEN json_valid(source.order_effects_json) THEN source.order_effects_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'planning_peer_changes','mutation',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.mutation AS doc, CASE WHEN path='$.edit.target' AND owner.record_table='waves' THEN 'uuid' WHEN path='$.edit.target' AND owner.record_table='projects' THEN 'proj_' WHEN path='$.edit.target' AND owner.record_table='tasks' THEN 'task_' WHEN path='$.edit.target' AND owner.record_table='task_comments' THEN 'uuid' WHEN parent='$.parents' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('comment_task') THEN 'task_' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('task_identifier') THEN 'label' WHEN parent='$.edit.change.value' AND typeof(key)='integer' AND json_extract(doc,'$.edit.change.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.edit.change.value.machine_id' AND json_extract(doc,'$.edit.change.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.edit.change.value.repository_id' AND json_extract(doc,'$.edit.change.field')='task_admission_authority' THEN 'repo_' END AS domain
 FROM canonical_planning_peer_changes owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.mutation AS doc FROM canonical_planning_peer_changes source,json_tree(CASE WHEN json_valid(source.mutation) THEN source.mutation END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'task_events','kind_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.kind_json AS doc, CASE WHEN path='$.pr_id' THEN 'pr_' WHEN path='$.steer_id' THEN 'uuid' WHEN path='$.invocation_id' THEN 'uuid' WHEN path='$.relation_id' THEN 'uuid' WHEN path='$.source_issue_id' THEN 'uuid' WHEN path='$.target_issue_id' THEN 'uuid' WHEN parent='$.lf_process_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.exec_ids' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.intent.issue_id' THEN 'task_or_uuid' WHEN path='$.intent.project_id' THEN 'project_or_uuid' WHEN path='$.intent.relation_id' THEN 'uuid' WHEN path='$.intent.team_id' THEN 'uuid' WHEN path='$.intent.state_id' THEN 'uuid' WHEN path='$.intent.identifier' THEN 'label' WHEN path='$.link.issue_id' THEN 'task_or_uuid' WHEN path='$.link.project_id' THEN 'project_or_uuid' WHEN path='$.link.relation_id' THEN 'uuid' WHEN path='$.link.team_id' THEN 'uuid' WHEN path='$.link.state_id' THEN 'uuid' WHEN path='$.link.identifier' THEN 'label' END AS domain
 FROM canonical_task_events owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.kind_json AS doc FROM canonical_task_events source,json_tree(CASE WHEN json_valid(source.kind_json) THEN source.kind_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'project_events','kind_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.kind_json AS doc, CASE WHEN path='$.event.pr_id' THEN 'pr_' WHEN path='$.event.steer_id' THEN 'uuid' WHEN path='$.event.invocation_id' THEN 'uuid' WHEN path='$.event.relation_id' THEN 'uuid' WHEN path='$.event.source_issue_id' THEN 'uuid' WHEN path='$.event.target_issue_id' THEN 'uuid' WHEN parent='$.event.lf_process_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.event.exec_ids' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.event.intent.issue_id' THEN 'task_or_uuid' WHEN path='$.event.intent.project_id' THEN 'project_or_uuid' WHEN path='$.event.intent.relation_id' THEN 'uuid' WHEN path='$.event.intent.team_id' THEN 'uuid' WHEN path='$.event.intent.state_id' THEN 'uuid' WHEN path='$.event.intent.identifier' THEN 'label' WHEN path='$.event.link.issue_id' THEN 'task_or_uuid' WHEN path='$.event.link.project_id' THEN 'project_or_uuid' WHEN path='$.event.link.relation_id' THEN 'uuid' WHEN path='$.event.link.team_id' THEN 'uuid' WHEN path='$.event.link.state_id' THEN 'uuid' WHEN path='$.event.link.identifier' THEN 'label' WHEN path='$.task_id' THEN 'task_' WHEN path='$.steer_id' THEN 'uuid' END AS domain
 FROM canonical_project_events owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.kind_json AS doc FROM canonical_project_events source,json_tree(CASE WHEN json_valid(source.kind_json) THEN source.kind_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'pr_landings','retired_supervisor_json',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.retired_supervisor_json AS doc, CASE WHEN path='$.home_id' THEN 'home_' END AS domain
 FROM canonical_pr_landings owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.retired_supervisor_json AS doc FROM canonical_pr_landings source,json_tree(CASE WHEN json_valid(source.retired_supervisor_json) THEN source.retired_supervisor_json END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'pm_project_name_cutover','body',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.body AS doc, CASE WHEN path='$.id' THEN 'uuid' WHEN parent='$.initiative_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.team_ids' AND typeof(key)='integer' THEN 'uuid' END AS domain
 FROM canonical_pm_project_name_cutover owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.body AS doc FROM canonical_pm_project_name_cutover source,json_tree(CASE WHEN json_valid(source.body) THEN source.body END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT INTO canonical_json
SELECT 'session_events','payload',source_rowid,fullkey,parent,domain,atom FROM (
 SELECT owner.source_rowid,leaf.fullkey,replace(leaf.fullkey,'"','') AS path,
 leaf.parent AS parent,leaf.key,leaf.atom,owner.payload AS doc, CASE WHEN path='$.flow_id' AND json_extract(doc,'$.type') IN ('legacy_flow','legacy_flow_review') THEN 'uuid' END AS domain
 FROM canonical_session_events owner JOIN (SELECT source_rowid,leaf.fullkey,leaf.key,leaf.atom,replace(leaf.fullkey,'"','') AS path,replace(leaf.path,'"','') AS parent,source.payload AS doc FROM canonical_session_events source,json_tree(CASE WHEN json_valid(source.payload) THEN source.payload END) leaf WHERE leaf.type='text') leaf USING(source_rowid)
) WHERE domain IS NOT NULL;

INSERT OR IGNORE INTO canonical_values(domain,original) SELECT domain,original FROM canonical_json;

WITH decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM canonical_values
), uuids AS (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
UPDATE canonical_values AS target SET canonical=(SELECT CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END FROM uuids WHERE uuids.domain=target.domain AND uuids.original=target.original);

UPDATE canonical_waves SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_waves.id),id),
    parent_wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_waves.parent_wave_id),parent_wave_id),
    superseded_by_wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_waves.superseded_by_wave_id),superseded_by_wave_id),
    project_activation_lf_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_waves.project_activation_lf_process_id),project_activation_lf_process_id),
    current_project_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='proj_' AND original=canonical_waves.current_project_id),current_project_id);

UPDATE canonical_processes SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_processes.id),id),
    trace_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_processes.trace_id),trace_id),
    parent_lf_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_processes.parent_lf_process_id),parent_lf_process_id),
    caller_agent_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_processes.caller_agent_process_id),caller_agent_process_id),
    attached_lf_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_processes.attached_lf_process_id),attached_lf_process_id),
    attachment_token=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_processes.attachment_token),attachment_token),
    caller_session_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='session' AND original=canonical_processes.caller_session_id),caller_session_id),
    agent_session_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='session' AND original=canonical_processes.agent_session_id),agent_session_id);

UPDATE canonical_agent_sessions SET wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_agent_sessions.wave_id),wave_id),
    agent_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_agent_sessions.agent_process_id),agent_process_id),
    transport_lf_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_agent_sessions.transport_lf_process_id),transport_lf_process_id),
    task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_agent_sessions.task_id),task_id),
    id=coalesce((SELECT canonical FROM canonical_values WHERE domain='session' AND original=canonical_agent_sessions.id),id);

UPDATE canonical_session_events SET lf_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_session_events.lf_process_id),lf_process_id),
    wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_session_events.wave_id),wave_id),
    agent_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_session_events.agent_process_id),agent_process_id),
    task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_session_events.task_id),task_id),
    session_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='session' AND original=canonical_session_events.session_id),session_id);

UPDATE canonical_session_activity SET attachment_token=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_session_activity.attachment_token),attachment_token),
    agent_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_session_activity.agent_process_id),agent_process_id),
    session_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='session' AND original=canonical_session_activity.session_id),session_id);

UPDATE canonical_flow_processes SET lf_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_flow_processes.lf_process_id),lf_process_id);

UPDATE canonical_flow_process_steps SET lf_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_flow_process_steps.lf_process_id),lf_process_id),
    flow_lf_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_flow_process_steps.flow_lf_process_id),flow_lf_process_id);

UPDATE canonical_task_workflows SET lf_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_task_workflows.lf_process_id),lf_process_id),
    task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_task_workflows.task_id),task_id);

UPDATE canonical_task_workflow_moves SET lf_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_task_workflow_moves.lf_process_id),lf_process_id),
    task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_task_workflow_moves.task_id),task_id);

UPDATE canonical_projects SET wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_projects.wave_id),wave_id),
    external_project_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_projects.external_project_id),external_project_id),
    id=coalesce((SELECT canonical FROM canonical_values WHERE domain='proj_' AND original=canonical_projects.id),id);

UPDATE canonical_tasks SET external_issue_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_tasks.external_issue_id),external_issue_id),
    planning_assignee=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_tasks.planning_assignee),planning_assignee),
    planning_team_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_tasks.planning_team_id),planning_team_id),
    id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_tasks.id),id),
    project_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='proj_' AND original=canonical_tasks.project_id),project_id),
    checkout_machine_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='home_' AND original=canonical_tasks.checkout_machine_id),checkout_machine_id),
    parent_pr_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='pr_' AND original=canonical_tasks.parent_pr_id),parent_pr_id),
    primary_session_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='session' AND original=canonical_tasks.primary_session_id),primary_session_id),
    issue_identifier=coalesce((SELECT canonical FROM canonical_values WHERE domain='label' AND original=canonical_tasks.issue_identifier),issue_identifier),
    repository_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='repo_' AND original=canonical_tasks.repository_id),repository_id);

UPDATE canonical_task_prs SET linear_attachment_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_task_prs.linear_attachment_id),linear_attachment_id),
    linear_comment_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_task_prs.linear_comment_id),linear_comment_id),
    task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_task_prs.task_id),task_id),
    id=coalesce((SELECT canonical FROM canonical_values WHERE domain='pr_' AND original=canonical_task_prs.id),id),
    parent_pr_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='pr_' AND original=canonical_task_prs.parent_pr_id),parent_pr_id);

UPDATE canonical_ci_incidents SET repair_lf_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_ci_incidents.repair_lf_process_id),repair_lf_process_id),
    task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_ci_incidents.task_id),task_id),
    pr_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='pr_' AND original=canonical_ci_incidents.pr_id),pr_id),
    landing_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='landing_' AND original=canonical_ci_incidents.landing_id),landing_id),
    repair_session_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='session' AND original=canonical_ci_incidents.repair_session_id),repair_session_id);

UPDATE canonical_task_linear_ingested_comments SET comment_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_task_linear_ingested_comments.comment_id),comment_id),
    task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_task_linear_ingested_comments.task_id),task_id);

UPDATE canonical_task_deletions SET wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_task_deletions.wave_id),wave_id),
    issue_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_task_deletions.issue_id),issue_id),
    identifier=coalesce((SELECT canonical FROM canonical_values WHERE domain='label' AND original=canonical_task_deletions.identifier),identifier);

UPDATE canonical_task_issue_identities SET wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_task_issue_identities.wave_id),wave_id),
    issue_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_task_issue_identities.issue_id),issue_id),
    identifier=coalesce((SELECT canonical FROM canonical_values WHERE domain='label' AND original=canonical_task_issue_identities.identifier),identifier);

UPDATE canonical_pm_projects SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_pm_projects.id),id);

UPDATE canonical_pm_items SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_pm_items.id),id),
    project_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_pm_items.project_id),project_id),
    team_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_pm_items.team_id),team_id),
    assignee=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_pm_items.assignee),assignee),
    claimed_project_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_pm_items.claimed_project_id),claimed_project_id),
    identifier=coalesce((SELECT canonical FROM canonical_values WHERE domain='label' AND original=canonical_pm_items.identifier),identifier);

UPDATE canonical_pm_wave_sync SET wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_pm_wave_sync.wave_id),wave_id),
    initiative=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_pm_wave_sync.initiative),initiative);

UPDATE canonical_pm_wave_projects SET wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_pm_wave_projects.wave_id),wave_id),
    project_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_pm_wave_projects.project_id),project_id);

UPDATE canonical_pm_issue_changes SET issue_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_pm_issue_changes.issue_id),issue_id);

UPDATE canonical_pm_project_name_cutover SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_pm_project_name_cutover.id),id);

UPDATE canonical_project_binding_imports SET wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_project_binding_imports.wave_id),wave_id);

UPDATE canonical_wave_workflows SET wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_wave_workflows.wave_id),wave_id);

UPDATE canonical_project_transitions SET wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_project_transitions.wave_id),wave_id),
    successor_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='project_or_uuid' AND original=canonical_project_transitions.successor_id),successor_id),
    predecessor_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='project_or_uuid' AND original=canonical_project_transitions.predecessor_id),predecessor_id);

UPDATE canonical_project_transition_items SET wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_project_transition_items.wave_id),wave_id),
    successor_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='project_or_uuid' AND original=canonical_project_transition_items.successor_id),successor_id),
    issue_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_or_uuid' AND original=canonical_project_transition_items.issue_id),issue_id);

UPDATE canonical_task_state_deliveries SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_task_state_deliveries.id),id),
    task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_task_state_deliveries.task_id),task_id);

UPDATE canonical_task_comments SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_task_comments.id),id),
    task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_task_comments.task_id),task_id);

UPDATE canonical_task_comment_deliveries SET comment_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_task_comment_deliveries.comment_id),comment_id);

UPDATE canonical_project_changes SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_project_changes.id),id),
    project_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='proj_' AND original=canonical_project_changes.project_id),project_id);

UPDATE canonical_task_changes SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_task_changes.id),id),
    task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_task_changes.task_id),task_id);

UPDATE canonical_planning_peer_changes SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_planning_peer_changes.id),id);

UPDATE canonical_planning_peer_observed SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_planning_peer_observed.id),id);

UPDATE canonical_planning_peer_routes SET mutation_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_planning_peer_routes.mutation_id),mutation_id);

UPDATE canonical_steer_adapters SET agent_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_steer_adapters.agent_process_id),agent_process_id);

UPDATE canonical_steers SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_steers.id),id),
    wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_steers.wave_id),wave_id),
    task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_steers.task_id),task_id),
    project_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='proj_' AND original=canonical_steers.project_id),project_id),
    session_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='session' AND original=canonical_steers.session_id),session_id);

UPDATE canonical_session_steers SET steer_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_session_steers.steer_id),steer_id),
    session_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='session' AND original=canonical_session_steers.session_id),session_id);

UPDATE canonical_steer_deliveries SET steer_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_steer_deliveries.steer_id),steer_id),
    batch_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_steer_deliveries.batch_id),batch_id),
    session_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='session' AND original=canonical_steer_deliveries.session_id),session_id);

UPDATE canonical_steer_batches SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_steer_batches.id),id),
    agent_process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_steer_batches.agent_process_id),agent_process_id),
    attachment_token=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_steer_batches.attachment_token),attachment_token),
    session_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='session' AND original=canonical_steer_batches.session_id),session_id);

UPDATE canonical_work_placements SET wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_work_placements.wave_id),wave_id),
    task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_work_placements.task_id),task_id),
    project_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='proj_' AND original=canonical_work_placements.project_id),project_id),
    machine_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='home_' AND original=canonical_work_placements.machine_id),machine_id);

UPDATE canonical_metric_instruments SET wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_metric_instruments.wave_id),wave_id);

UPDATE canonical_metric_observations SET wave_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_metric_observations.wave_id),wave_id);

UPDATE canonical_linear_team_checks SET team_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_linear_team_checks.team_id),team_id);

UPDATE canonical_relocated_task_processes SET process_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='uuid' AND original=canonical_relocated_task_processes.process_id),process_id),
    task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_relocated_task_processes.task_id),task_id);

UPDATE canonical_task_events SET task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_task_events.task_id),task_id);

UPDATE canonical_pr_landings SET task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_pr_landings.task_id),task_id),
    supervisor_machine_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='home_' AND original=canonical_pr_landings.supervisor_machine_id),supervisor_machine_id),
    id=coalesce((SELECT canonical FROM canonical_values WHERE domain='landing_' AND original=canonical_pr_landings.id),id);

UPDATE canonical_task_linear_observations SET task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_task_linear_observations.task_id),task_id);

UPDATE canonical_task_creation_intents SET task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_task_creation_intents.task_id),task_id),
    project_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='proj_' AND original=canonical_task_creation_intents.project_id),project_id),
    repository_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='repo_' AND original=canonical_task_creation_intents.repository_id),repository_id);

UPDATE canonical_task_admissions SET task_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='task_' AND original=canonical_task_admissions.task_id),task_id),
    machine_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='home_' AND original=canonical_task_admissions.machine_id),machine_id);

UPDATE canonical_project_events SET project_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='proj_' AND original=canonical_project_events.project_id),project_id);

UPDATE canonical_machines SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='home_' AND original=canonical_machines.id),id);

UPDATE canonical_task_pr_repair_incidents SET task_pr_id=coalesce((SELECT canonical FROM canonical_values WHERE domain='pr_' AND original=canonical_task_pr_repair_incidents.task_pr_id),task_pr_id);

UPDATE canonical_repository_plans SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='repo_' AND original=canonical_repository_plans.id),id);

UPDATE canonical_tool_responses SET id=coalesce((SELECT canonical FROM canonical_values WHERE domain='response_' AND original=canonical_tool_responses.id),id),
    work_id=coalesce((SELECT canonical FROM canonical_values WHERE domain=CASE work_kind WHEN 'wave' THEN 'uuid' WHEN 'project' THEN 'proj_' WHEN 'task' THEN 'task_' END AND original=canonical_tool_responses.work_id),work_id);

UPDATE canonical_planning_peer_conflicts SET object_id=coalesce((SELECT canonical FROM canonical_values WHERE domain=CASE kind WHEN 'waves' THEN 'uuid' WHEN 'projects' THEN 'proj_' WHEN 'tasks' THEN 'task_' WHEN 'task_comments' THEN 'uuid' END AND original=canonical_planning_peer_conflicts.object_id),object_id);

CREATE TEMP TABLE canonical_identity_conflicts(domain TEXT,first TEXT,second TEXT);
CREATE TEMP TRIGGER canonical_identity_conflict BEFORE INSERT ON canonical_identity_conflicts
BEGIN SELECT RAISE(ABORT,'canonical identity collision: '||NEW.domain||': '||NEW.first||' / '||NEW.second); END;
INSERT INTO canonical_identity_conflicts
SELECT j.owner||'.'||j.column_name||' '||j.parent,min(j.original),max(j.original)
FROM canonical_json j JOIN canonical_values v USING(domain,original)
WHERE substr(j.path,-1)=']'
GROUP BY j.owner,j.column_name,j.source_rowid,j.parent,v.canonical
HAVING count(DISTINCT j.original)>1;


WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='tasks' AND j.column_name='admission_authority' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,admission_authority FROM canonical_tasks WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_tasks SET admission_authority=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_tasks.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='projects' AND j.column_name='planning_initiatives' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,planning_initiatives FROM canonical_projects WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_projects SET planning_initiatives=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_projects.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='projects' AND j.column_name='planning_teams' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,planning_teams FROM canonical_projects WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_projects SET planning_teams=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_projects.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='pm_projects' AND j.column_name='initiative_ids' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,initiative_ids FROM canonical_pm_projects WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_pm_projects SET initiative_ids=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_pm_projects.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='pm_projects' AND j.column_name='team_ids' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,team_ids FROM canonical_pm_projects WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_pm_projects SET team_ids=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_pm_projects.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='pm_projects' AND j.column_name='task_order_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,task_order_json FROM canonical_pm_projects WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_pm_projects SET task_order_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_pm_projects.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='task_comment_deliveries' AND j.column_name='comment_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,comment_json FROM canonical_task_comment_deliveries WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_task_comment_deliveries SET comment_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_task_comment_deliveries.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='task_comment_deliveries' AND j.column_name='conflicting_comment_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,conflicting_comment_json FROM canonical_task_comment_deliveries WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_task_comment_deliveries SET conflicting_comment_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_task_comment_deliveries.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='project_transitions' AND j.column_name='local_plan_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,local_plan_json FROM canonical_project_transitions WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_project_transitions SET local_plan_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_project_transitions.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='project_transitions' AND j.column_name='export_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,export_json FROM canonical_project_transitions WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_project_transitions SET export_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_project_transitions.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='task_creation_intents' AND j.column_name='export_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,export_json FROM canonical_task_creation_intents WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_task_creation_intents SET export_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_task_creation_intents.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='task_changes' AND j.column_name='value_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,value_json FROM canonical_task_changes WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_task_changes SET value_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_task_changes.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='task_changes' AND j.column_name='base_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,base_json FROM canonical_task_changes WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_task_changes SET base_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_task_changes.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='task_changes' AND j.column_name='conflict_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,conflict_json FROM canonical_task_changes WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_task_changes SET conflict_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_task_changes.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='project_changes' AND j.column_name='value_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,value_json FROM canonical_project_changes WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_project_changes SET value_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_project_changes.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='project_changes' AND j.column_name='base_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,base_json FROM canonical_project_changes WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_project_changes SET base_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_project_changes.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='project_changes' AND j.column_name='conflict_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,conflict_json FROM canonical_project_changes WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_project_changes SET conflict_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_project_changes.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='project_changes' AND j.column_name='order_effects_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,order_effects_json FROM canonical_project_changes WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_project_changes SET order_effects_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_project_changes.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='planning_peer_changes' AND j.column_name='mutation' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,mutation FROM canonical_planning_peer_changes WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_planning_peer_changes SET mutation=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_planning_peer_changes.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='task_events' AND j.column_name='kind_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,kind_json FROM canonical_task_events WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_task_events SET kind_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_task_events.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='project_events' AND j.column_name='kind_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,kind_json FROM canonical_project_events WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_project_events SET kind_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_project_events.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='pr_landings' AND j.column_name='retired_supervisor_json' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,retired_supervisor_json FROM canonical_pr_landings WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_pr_landings SET retired_supervisor_json=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_pr_landings.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='pm_project_name_cutover' AND j.column_name='body' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,body FROM canonical_pm_project_name_cutover WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_pm_project_name_cutover SET body=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_pm_project_name_cutover.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

WITH RECURSIVE patches AS (
 SELECT j.source_rowid,j.path,v.canonical,row_number() OVER(PARTITION BY j.source_rowid ORDER BY j.path) AS n
 FROM canonical_json j JOIN canonical_values v USING(domain,original)
 WHERE j.owner='session_events' AND j.column_name='payload' AND v.original!=v.canonical
), rewritten(source_rowid,n,doc) AS (
 SELECT source_rowid,0,payload FROM canonical_session_events WHERE source_rowid IN (SELECT source_rowid FROM patches)
 UNION ALL SELECT r.source_rowid,p.n,json_set(r.doc,p.path,p.canonical)
 FROM rewritten r JOIN patches p ON p.source_rowid=r.source_rowid AND p.n=r.n+1
)
UPDATE canonical_session_events SET payload=(SELECT doc FROM rewritten r WHERE r.source_rowid=canonical_session_events.source_rowid ORDER BY n DESC LIMIT 1)
WHERE source_rowid IN (SELECT source_rowid FROM patches);

UPDATE canonical_planning_peer_changes SET object_id=json_extract(mutation,'$.edit.target');

INSERT INTO canonical_identity_conflicts SELECT 'agent_sessions (agent_sessions_primary_wave)',min((SELECT wave_id FROM agent_sessions original WHERE original.rowid=c.source_rowid)),max((SELECT wave_id FROM agent_sessions original WHERE original.rowid=c.source_rowid)) FROM canonical_agent_sessions c WHERE (primary_scope = 'wave' AND completed_at IS NULL) AND (wave_id) IS NOT NULL GROUP BY wave_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'agent_sessions (agent_sessions_primary_repository)',min((SELECT wave_id FROM agent_sessions original WHERE original.rowid=c.source_rowid)),max((SELECT wave_id FROM agent_sessions original WHERE original.rowid=c.source_rowid)) FROM canonical_agent_sessions c WHERE (primary_scope = 'repository' AND completed_at IS NULL) AND (repo) IS NOT NULL GROUP BY repo HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'agent_sessions (sqlite_autoindex_agent_sessions_2)',min((SELECT wave_id FROM agent_sessions original WHERE original.rowid=c.source_rowid)),max((SELECT wave_id FROM agent_sessions original WHERE original.rowid=c.source_rowid)) FROM canonical_agent_sessions c WHERE (1) AND (current_capture) IS NOT NULL GROUP BY current_capture HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'agent_sessions (sqlite_autoindex_agent_sessions_1)',min((SELECT id FROM agent_sessions original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM agent_sessions original WHERE original.rowid=c.source_rowid)) FROM canonical_agent_sessions c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'ci_incidents (sqlite_autoindex_ci_incidents_1)',min((SELECT repair_lf_process_id FROM ci_incidents original WHERE original.rowid=c.source_rowid)),max((SELECT repair_lf_process_id FROM ci_incidents original WHERE original.rowid=c.source_rowid)) FROM canonical_ci_incidents c WHERE (1) AND (identity) IS NOT NULL GROUP BY identity HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'flow_process_steps (sqlite_autoindex_flow_process_steps_1)',min((SELECT lf_process_id FROM flow_process_steps original WHERE original.rowid=c.source_rowid)),max((SELECT lf_process_id FROM flow_process_steps original WHERE original.rowid=c.source_rowid)) FROM canonical_flow_process_steps c WHERE (1) AND (lf_process_id) IS NOT NULL GROUP BY lf_process_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'flow_processes (sqlite_autoindex_flow_processes_1)',min((SELECT lf_process_id FROM flow_processes original WHERE original.rowid=c.source_rowid)),max((SELECT lf_process_id FROM flow_processes original WHERE original.rowid=c.source_rowid)) FROM canonical_flow_processes c WHERE (1) AND (lf_process_id) IS NOT NULL GROUP BY lf_process_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'linear_team_checks (sqlite_autoindex_linear_team_checks_1)',min((SELECT team_id FROM linear_team_checks original WHERE original.rowid=c.source_rowid)),max((SELECT team_id FROM linear_team_checks original WHERE original.rowid=c.source_rowid)) FROM canonical_linear_team_checks c WHERE (1) AND (team_id) IS NOT NULL GROUP BY team_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'machines (idx_machines_route)',min((SELECT id FROM machines original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM machines original WHERE original.rowid=c.source_rowid)) FROM canonical_machines c WHERE (route='local' OR label IS NOT NULL) AND (route) IS NOT NULL GROUP BY route HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'machines (idx_machines_label)',min((SELECT id FROM machines original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM machines original WHERE original.rowid=c.source_rowid)) FROM canonical_machines c WHERE (label IS NOT NULL) AND (label) IS NOT NULL GROUP BY label HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'machines (sqlite_autoindex_machines_1)',min((SELECT id FROM machines original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM machines original WHERE original.rowid=c.source_rowid)) FROM canonical_machines c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'metric_instruments (sqlite_autoindex_metric_instruments_1)',min((SELECT wave_id FROM metric_instruments original WHERE original.rowid=c.source_rowid)),max((SELECT wave_id FROM metric_instruments original WHERE original.rowid=c.source_rowid)) FROM canonical_metric_instruments c WHERE (1) AND (wave_id) IS NOT NULL AND (metric_id) IS NOT NULL GROUP BY wave_id,metric_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'metric_observations (sqlite_autoindex_metric_observations_1)',min((SELECT wave_id FROM metric_observations original WHERE original.rowid=c.source_rowid)),max((SELECT wave_id FROM metric_observations original WHERE original.rowid=c.source_rowid)) FROM canonical_metric_observations c WHERE (1) AND (observation_id) IS NOT NULL GROUP BY observation_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'planning_peer_changes (sqlite_autoindex_planning_peer_changes_1)',min((SELECT id FROM planning_peer_changes original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM planning_peer_changes original WHERE original.rowid=c.source_rowid)) FROM canonical_planning_peer_changes c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'planning_peer_conflicts (sqlite_autoindex_planning_peer_conflicts_1)',min((SELECT object_id FROM planning_peer_conflicts original WHERE original.rowid=c.source_rowid)),max((SELECT object_id FROM planning_peer_conflicts original WHERE original.rowid=c.source_rowid)) FROM canonical_planning_peer_conflicts c WHERE (1) AND (kind) IS NOT NULL AND (object_id) IS NOT NULL AND (reason) IS NOT NULL GROUP BY kind,object_id,reason HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'planning_peer_observed (sqlite_autoindex_planning_peer_observed_1)',min((SELECT id FROM planning_peer_observed original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM planning_peer_observed original WHERE original.rowid=c.source_rowid)) FROM canonical_planning_peer_observed c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'planning_peer_routes (sqlite_autoindex_planning_peer_routes_1)',min((SELECT mutation_id FROM planning_peer_routes original WHERE original.rowid=c.source_rowid)),max((SELECT mutation_id FROM planning_peer_routes original WHERE original.rowid=c.source_rowid)) FROM canonical_planning_peer_routes c WHERE (1) AND (repo) IS NOT NULL AND (destination) IS NOT NULL AND (mutation_id) IS NOT NULL GROUP BY repo,destination,mutation_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'pm_issue_changes (sqlite_autoindex_pm_issue_changes_1)',min((SELECT issue_id FROM pm_issue_changes original WHERE original.rowid=c.source_rowid)),max((SELECT issue_id FROM pm_issue_changes original WHERE original.rowid=c.source_rowid)) FROM canonical_pm_issue_changes c WHERE (1) AND (issue_id) IS NOT NULL GROUP BY issue_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'pm_items (sqlite_autoindex_pm_items_1)',min((SELECT id FROM pm_items original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM pm_items original WHERE original.rowid=c.source_rowid)) FROM canonical_pm_items c WHERE (1) AND (repo) IS NOT NULL AND (provider) IS NOT NULL AND (id) IS NOT NULL GROUP BY repo,provider,id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'pm_project_name_cutover (sqlite_autoindex_pm_project_name_cutover_1)',min((SELECT id FROM pm_project_name_cutover original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM pm_project_name_cutover original WHERE original.rowid=c.source_rowid)) FROM canonical_pm_project_name_cutover c WHERE (1) AND (repo) IS NOT NULL AND (provider) IS NOT NULL AND (id) IS NOT NULL GROUP BY repo,provider,id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'pm_projects (sqlite_autoindex_pm_projects_1)',min((SELECT id FROM pm_projects original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM pm_projects original WHERE original.rowid=c.source_rowid)) FROM canonical_pm_projects c WHERE (1) AND (repo) IS NOT NULL AND (provider) IS NOT NULL AND (id) IS NOT NULL GROUP BY repo,provider,id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'pm_wave_projects (sqlite_autoindex_pm_wave_projects_1)',min((SELECT wave_id FROM pm_wave_projects original WHERE original.rowid=c.source_rowid)),max((SELECT wave_id FROM pm_wave_projects original WHERE original.rowid=c.source_rowid)) FROM canonical_pm_wave_projects c WHERE (1) AND (wave_id) IS NOT NULL AND (project_id) IS NOT NULL GROUP BY wave_id,project_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'pm_wave_sync (sqlite_autoindex_pm_wave_sync_1)',min((SELECT wave_id FROM pm_wave_sync original WHERE original.rowid=c.source_rowid)),max((SELECT wave_id FROM pm_wave_sync original WHERE original.rowid=c.source_rowid)) FROM canonical_pm_wave_sync c WHERE (1) AND (wave_id) IS NOT NULL GROUP BY wave_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'pr_landings (idx_pr_landings_active)',min((SELECT task_id FROM pr_landings original WHERE original.rowid=c.source_rowid)),max((SELECT task_id FROM pr_landings original WHERE original.rowid=c.source_rowid)) FROM canonical_pr_landings c WHERE (state IN ('watching', 'repairing')) AND (repo) IS NOT NULL AND (pr_number) IS NOT NULL GROUP BY repo, pr_number HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'pr_landings (sqlite_autoindex_pr_landings_1)',min((SELECT id FROM pr_landings original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM pr_landings original WHERE original.rowid=c.source_rowid)) FROM canonical_pr_landings c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'processes (sqlite_autoindex_processes_1)',min((SELECT id FROM processes original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM processes original WHERE original.rowid=c.source_rowid)) FROM canonical_processes c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'project_binding_imports (sqlite_autoindex_project_binding_imports_1)',min((SELECT wave_id FROM project_binding_imports original WHERE original.rowid=c.source_rowid)),max((SELECT wave_id FROM project_binding_imports original WHERE original.rowid=c.source_rowid)) FROM canonical_project_binding_imports c WHERE (1) AND (wave_id) IS NOT NULL GROUP BY wave_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'project_changes (sqlite_autoindex_project_changes_1)',min((SELECT id FROM project_changes original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM project_changes original WHERE original.rowid=c.source_rowid)) FROM canonical_project_changes c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'project_transition_items (sqlite_autoindex_project_transition_items_1)',min((SELECT wave_id FROM project_transition_items original WHERE original.rowid=c.source_rowid)),max((SELECT wave_id FROM project_transition_items original WHERE original.rowid=c.source_rowid)) FROM canonical_project_transition_items c WHERE (1) AND (wave_id) IS NOT NULL AND (successor_id) IS NOT NULL AND (issue_id) IS NOT NULL GROUP BY wave_id,successor_id,issue_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'project_transitions (project_transitions_pending)',min((SELECT wave_id FROM project_transitions original WHERE original.rowid=c.source_rowid)),max((SELECT wave_id FROM project_transitions original WHERE original.rowid=c.source_rowid)) FROM canonical_project_transitions c WHERE (settled_at IS NULL) AND (wave_id) IS NOT NULL GROUP BY wave_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'project_transitions (sqlite_autoindex_project_transitions_1)',min((SELECT wave_id FROM project_transitions original WHERE original.rowid=c.source_rowid)),max((SELECT wave_id FROM project_transitions original WHERE original.rowid=c.source_rowid)) FROM canonical_project_transitions c WHERE (1) AND (wave_id) IS NOT NULL AND (successor_id) IS NOT NULL GROUP BY wave_id,successor_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'projects (sqlite_autoindex_projects_2)',min((SELECT external_project_id FROM projects original WHERE original.rowid=c.source_rowid)),max((SELECT external_project_id FROM projects original WHERE original.rowid=c.source_rowid)) FROM canonical_projects c WHERE (1) AND (external_project_id) IS NOT NULL GROUP BY external_project_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'projects (sqlite_autoindex_projects_1)',min((SELECT id FROM projects original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM projects original WHERE original.rowid=c.source_rowid)) FROM canonical_projects c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'relocated_task_processes (sqlite_autoindex_relocated_task_processes_1)',min((SELECT task_id FROM relocated_task_processes original WHERE original.rowid=c.source_rowid)),max((SELECT task_id FROM relocated_task_processes original WHERE original.rowid=c.source_rowid)) FROM canonical_relocated_task_processes c WHERE (1) AND (task_id) IS NOT NULL AND (process_id) IS NOT NULL GROUP BY task_id,process_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'repository_plans (repository_selected_plan)',min((SELECT id FROM repository_plans original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM repository_plans original WHERE original.rowid=c.source_rowid)) FROM canonical_repository_plans c WHERE (selected=1) AND (repo) IS NOT NULL GROUP BY repo HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'repository_plans (sqlite_autoindex_repository_plans_1)',min((SELECT id FROM repository_plans original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM repository_plans original WHERE original.rowid=c.source_rowid)) FROM canonical_repository_plans c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'session_activity (sqlite_autoindex_session_activity_1)',min((SELECT session_id FROM session_activity original WHERE original.rowid=c.source_rowid)),max((SELECT session_id FROM session_activity original WHERE original.rowid=c.source_rowid)) FROM canonical_session_activity c WHERE (1) AND (session_id) IS NOT NULL GROUP BY session_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'session_events (session_capture_key)',min((SELECT lf_process_id FROM session_events original WHERE original.rowid=c.source_rowid)),max((SELECT lf_process_id FROM session_events original WHERE original.rowid=c.source_rowid)) FROM canonical_session_events c WHERE (kind='captured') AND (receipt_key) IS NOT NULL GROUP BY receipt_key HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'session_events (session_event_receipt)',min((SELECT lf_process_id FROM session_events original WHERE original.rowid=c.source_rowid)),max((SELECT lf_process_id FROM session_events original WHERE original.rowid=c.source_rowid)) FROM canonical_session_events c WHERE (1) AND (
    session_id) IS NOT NULL AND (COALESCE(provider_thread,'')) IS NOT NULL AND (COALESCE(provider_turn,'')) IS NOT NULL AND (kind) IS NOT NULL AND (receipt_key) IS NOT NULL GROUP BY
    session_id, COALESCE(provider_thread,''), COALESCE(provider_turn,''), kind, receipt_key HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'session_steers (sqlite_autoindex_session_steers_1)',min((SELECT session_id FROM session_steers original WHERE original.rowid=c.source_rowid)),max((SELECT session_id FROM session_steers original WHERE original.rowid=c.source_rowid)) FROM canonical_session_steers c WHERE (1) AND (session_id) IS NOT NULL AND (steer_id) IS NOT NULL GROUP BY session_id,steer_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'steer_adapters (sqlite_autoindex_steer_adapters_2)',min((SELECT agent_process_id FROM steer_adapters original WHERE original.rowid=c.source_rowid)),max((SELECT agent_process_id FROM steer_adapters original WHERE original.rowid=c.source_rowid)) FROM canonical_steer_adapters c WHERE (1) AND (capability) IS NOT NULL GROUP BY capability HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'steer_adapters (sqlite_autoindex_steer_adapters_1)',min((SELECT agent_process_id FROM steer_adapters original WHERE original.rowid=c.source_rowid)),max((SELECT agent_process_id FROM steer_adapters original WHERE original.rowid=c.source_rowid)) FROM canonical_steer_adapters c WHERE (1) AND (agent_process_id) IS NOT NULL GROUP BY agent_process_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'steer_batches (steer_batches_boundary)',min((SELECT agent_process_id FROM steer_batches original WHERE original.rowid=c.source_rowid)),max((SELECT agent_process_id FROM steer_batches original WHERE original.rowid=c.source_rowid)) FROM canonical_steer_batches c WHERE (evidence != 'cancelled') AND (agent_process_id) IS NOT NULL AND (boundary) IS NOT NULL GROUP BY agent_process_id,boundary HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'steer_batches (sqlite_autoindex_steer_batches_1)',min((SELECT id FROM steer_batches original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM steer_batches original WHERE original.rowid=c.source_rowid)) FROM canonical_steer_batches c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'steer_deliveries (sqlite_autoindex_steer_deliveries_1)',min((SELECT session_id FROM steer_deliveries original WHERE original.rowid=c.source_rowid)),max((SELECT session_id FROM steer_deliveries original WHERE original.rowid=c.source_rowid)) FROM canonical_steer_deliveries c WHERE (1) AND (session_id) IS NOT NULL AND (steer_id) IS NOT NULL AND (attempt) IS NOT NULL GROUP BY session_id,steer_id,attempt HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'steers (sqlite_autoindex_steers_2)',min((SELECT id FROM steers original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM steers original WHERE original.rowid=c.source_rowid)) FROM canonical_steers c WHERE (1) AND (source) IS NOT NULL AND (revision) IS NOT NULL GROUP BY source,revision HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'steers (sqlite_autoindex_steers_1)',min((SELECT id FROM steers original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM steers original WHERE original.rowid=c.source_rowid)) FROM canonical_steers c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_admissions (sqlite_autoindex_task_admissions_1)',min((SELECT task_id FROM task_admissions original WHERE original.rowid=c.source_rowid)),max((SELECT task_id FROM task_admissions original WHERE original.rowid=c.source_rowid)) FROM canonical_task_admissions c WHERE (1) AND (task_id) IS NOT NULL GROUP BY task_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_changes (sqlite_autoindex_task_changes_1)',min((SELECT id FROM task_changes original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM task_changes original WHERE original.rowid=c.source_rowid)) FROM canonical_task_changes c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_comment_deliveries (sqlite_autoindex_task_comment_deliveries_1)',min((SELECT comment_id FROM task_comment_deliveries original WHERE original.rowid=c.source_rowid)),max((SELECT comment_id FROM task_comment_deliveries original WHERE original.rowid=c.source_rowid)) FROM canonical_task_comment_deliveries c WHERE (1) AND (comment_id) IS NOT NULL GROUP BY comment_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_comments (sqlite_autoindex_task_comments_1)',min((SELECT id FROM task_comments original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM task_comments original WHERE original.rowid=c.source_rowid)) FROM canonical_task_comments c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_creation_intents (sqlite_autoindex_task_creation_intents_1)',min((SELECT task_id FROM task_creation_intents original WHERE original.rowid=c.source_rowid)),max((SELECT task_id FROM task_creation_intents original WHERE original.rowid=c.source_rowid)) FROM canonical_task_creation_intents c WHERE (1) AND (task_id) IS NOT NULL GROUP BY task_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_deletions (sqlite_autoindex_task_deletions_1)',min((SELECT wave_id FROM task_deletions original WHERE original.rowid=c.source_rowid)),max((SELECT wave_id FROM task_deletions original WHERE original.rowid=c.source_rowid)) FROM canonical_task_deletions c WHERE (1) AND (wave_id) IS NOT NULL AND (issue_id) IS NOT NULL GROUP BY wave_id,issue_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_issue_identities (sqlite_autoindex_task_issue_identities_1)',min((SELECT issue_id FROM task_issue_identities original WHERE original.rowid=c.source_rowid)),max((SELECT issue_id FROM task_issue_identities original WHERE original.rowid=c.source_rowid)) FROM canonical_task_issue_identities c WHERE (1) AND (issue_id) IS NOT NULL GROUP BY issue_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_linear_ingested_comments (sqlite_autoindex_task_linear_ingested_comments_1)',min((SELECT task_id FROM task_linear_ingested_comments original WHERE original.rowid=c.source_rowid)),max((SELECT task_id FROM task_linear_ingested_comments original WHERE original.rowid=c.source_rowid)) FROM canonical_task_linear_ingested_comments c WHERE (1) AND (task_id) IS NOT NULL AND (comment_id) IS NOT NULL GROUP BY task_id,comment_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_linear_observations (sqlite_autoindex_task_linear_observations_1)',min((SELECT task_id FROM task_linear_observations original WHERE original.rowid=c.source_rowid)),max((SELECT task_id FROM task_linear_observations original WHERE original.rowid=c.source_rowid)) FROM canonical_task_linear_observations c WHERE (1) AND (task_id) IS NOT NULL GROUP BY task_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_pr_repair_incidents (sqlite_autoindex_task_pr_repair_incidents_1)',min((SELECT task_pr_id FROM task_pr_repair_incidents original WHERE original.rowid=c.source_rowid)),max((SELECT task_pr_id FROM task_pr_repair_incidents original WHERE original.rowid=c.source_rowid)) FROM canonical_task_pr_repair_incidents c WHERE (1) AND (task_pr_id) IS NOT NULL AND (kind) IS NOT NULL GROUP BY task_pr_id,kind HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_prs (idx_task_prs_current_branch)',min((SELECT linear_attachment_id FROM task_prs original WHERE original.rowid=c.source_rowid)),max((SELECT linear_attachment_id FROM task_prs original WHERE original.rowid=c.source_rowid)) FROM canonical_task_prs c WHERE (historical=0) AND (branch) IS NOT NULL GROUP BY branch HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_prs (idx_task_prs_current)',min((SELECT task_id FROM task_prs original WHERE original.rowid=c.source_rowid)),max((SELECT task_id FROM task_prs original WHERE original.rowid=c.source_rowid)) FROM canonical_task_prs c WHERE (historical=0) AND (task_id) IS NOT NULL GROUP BY task_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_prs (sqlite_autoindex_task_prs_2)',min((SELECT task_id FROM task_prs original WHERE original.rowid=c.source_rowid)),max((SELECT task_id FROM task_prs original WHERE original.rowid=c.source_rowid)) FROM canonical_task_prs c WHERE (1) AND (task_id) IS NOT NULL AND (sequence) IS NOT NULL GROUP BY task_id,sequence HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_prs (sqlite_autoindex_task_prs_1)',min((SELECT id FROM task_prs original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM task_prs original WHERE original.rowid=c.source_rowid)) FROM canonical_task_prs c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_state_deliveries (sqlite_autoindex_task_state_deliveries_1)',min((SELECT id FROM task_state_deliveries original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM task_state_deliveries original WHERE original.rowid=c.source_rowid)) FROM canonical_task_state_deliveries c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'task_workflows (sqlite_autoindex_task_workflows_1)',min((SELECT task_id FROM task_workflows original WHERE original.rowid=c.source_rowid)),max((SELECT task_id FROM task_workflows original WHERE original.rowid=c.source_rowid)) FROM canonical_task_workflows c WHERE (1) AND (task_id) IS NOT NULL GROUP BY task_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'tasks (idx_tasks_worktree)',min((SELECT external_issue_id FROM tasks original WHERE original.rowid=c.source_rowid)),max((SELECT external_issue_id FROM tasks original WHERE original.rowid=c.source_rowid)) FROM canonical_tasks c WHERE (1) AND (worktree) IS NOT NULL GROUP BY worktree HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'tasks (sqlite_autoindex_tasks_2)',min((SELECT external_issue_id FROM tasks original WHERE original.rowid=c.source_rowid)),max((SELECT external_issue_id FROM tasks original WHERE original.rowid=c.source_rowid)) FROM canonical_tasks c WHERE (1) AND (external_issue_id) IS NOT NULL GROUP BY external_issue_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'tasks (sqlite_autoindex_tasks_1)',min((SELECT id FROM tasks original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM tasks original WHERE original.rowid=c.source_rowid)) FROM canonical_tasks c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'tool_responses (sqlite_autoindex_tool_responses_2)',min((SELECT work_id FROM tool_responses original WHERE original.rowid=c.source_rowid)),max((SELECT work_id FROM tool_responses original WHERE original.rowid=c.source_rowid)) FROM canonical_tool_responses c WHERE (1) AND (work_kind) IS NOT NULL AND (work_id) IS NOT NULL AND (request_id) IS NOT NULL GROUP BY work_kind,work_id,request_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'tool_responses (sqlite_autoindex_tool_responses_1)',min((SELECT id FROM tool_responses original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM tool_responses original WHERE original.rowid=c.source_rowid)) FROM canonical_tool_responses c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'wave_workflows (sqlite_autoindex_wave_workflows_1)',min((SELECT wave_id FROM wave_workflows original WHERE original.rowid=c.source_rowid)),max((SELECT wave_id FROM wave_workflows original WHERE original.rowid=c.source_rowid)) FROM canonical_wave_workflows c WHERE (1) AND (wave_id) IS NOT NULL AND (name) IS NOT NULL GROUP BY wave_id,name HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'waves (idx_waves_active_locator)',min((SELECT id FROM waves original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM waves original WHERE original.rowid=c.source_rowid)) FROM canonical_waves c WHERE (retired_at IS NULL) AND (repo) IS NOT NULL AND (ifnull(parent_wave_id,'')) IS NOT NULL AND (name) IS NOT NULL GROUP BY repo,ifnull(parent_wave_id,''),name HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'waves (sqlite_autoindex_waves_1)',min((SELECT id FROM waves original WHERE original.rowid=c.source_rowid)),max((SELECT id FROM waves original WHERE original.rowid=c.source_rowid)) FROM canonical_waves c WHERE (1) AND (id) IS NOT NULL GROUP BY id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'work_placements (sqlite_autoindex_work_placements_3)',min((SELECT task_id FROM work_placements original WHERE original.rowid=c.source_rowid)),max((SELECT task_id FROM work_placements original WHERE original.rowid=c.source_rowid)) FROM canonical_work_placements c WHERE (1) AND (task_id) IS NOT NULL GROUP BY task_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'work_placements (sqlite_autoindex_work_placements_2)',min((SELECT project_id FROM work_placements original WHERE original.rowid=c.source_rowid)),max((SELECT project_id FROM work_placements original WHERE original.rowid=c.source_rowid)) FROM canonical_work_placements c WHERE (1) AND (project_id) IS NOT NULL GROUP BY project_id HAVING count(*)>1;

INSERT INTO canonical_identity_conflicts SELECT 'work_placements (sqlite_autoindex_work_placements_1)',min((SELECT wave_id FROM work_placements original WHERE original.rowid=c.source_rowid)),max((SELECT wave_id FROM work_placements original WHERE original.rowid=c.source_rowid)) FROM canonical_work_placements c WHERE (1) AND (wave_id) IS NOT NULL GROUP BY wave_id HAVING count(*)>1;

CREATE TEMP VIEW canonical_planning_heads AS
SELECT 0 AS converted,p.rowid AS source_rowid,p.id,p.object_id,p.field,p.clock FROM planning_peer_changes p
WHERE NOT EXISTS(SELECT 1 FROM planning_peer_changes child,json_each(child.mutation,'$.parents') parent WHERE parent.value=p.id AND child.object_id=p.object_id AND child.field=p.field)
UNION ALL
SELECT 1,p.source_rowid,p.id,p.object_id,p.field,p.clock FROM canonical_planning_peer_changes p
WHERE NOT EXISTS(SELECT 1 FROM canonical_planning_peer_changes child,json_each(child.mutation,'$.parents') parent WHERE parent.value=p.id AND child.object_id=p.object_id AND child.field=p.field);


INSERT INTO canonical_identity_conflicts
WITH winners AS (SELECT *,row_number() OVER(PARTITION BY converted,object_id,field ORDER BY clock DESC,id DESC) AS rank FROM canonical_planning_heads)
SELECT 'planning winner',old.id,new.id FROM winners old
JOIN canonical_planning_peer_changes mapped ON mapped.source_rowid=old.source_rowid
JOIN winners new ON new.converted=1 AND new.object_id=mapped.object_id AND new.field=mapped.field AND new.rank=1
WHERE old.converted=0 AND old.rank=1 AND old.source_rowid!=new.source_rowid;
DROP VIEW canonical_planning_heads;

DROP TRIGGER task_pr_repair_incidents_are_immutable;

DROP TRIGGER preserve_capture_update;

DROP TRIGGER validate_session_capture;

DROP TRIGGER task_first_conversation_bind;

DROP TRIGGER selected_project_wave_update;

DROP TRIGGER validate_conversation_ancestry_update;

DROP TRIGGER task_workflow_graph_is_fixed;

DROP TRIGGER task_workflow_moves_are_append_only;

DROP TRIGGER flow_processes_are_append_only;

DROP TRIGGER flow_process_steps_are_append_only;

DROP TRIGGER pr_landings_no_machine_update;

DROP TRIGGER selected_project_owner_update;

DROP TRIGGER validate_task_started_update;

DROP TRIGGER task_prs_preserve_merged_at;

DROP TRIGGER task_pr_history_immutable;

DROP TRIGGER task_pr_placement_update;

DROP TRIGGER task_admissions_immutable;

DROP TRIGGER task_pr_history_no_mark;

DROP TRIGGER task_placement_to_pr;

DROP TRIGGER task_repository_is_fixed;

DROP TRIGGER validate_task_repository_update;

DROP TRIGGER validate_project_task_repositories;

DROP TRIGGER task_session_membership;

DROP TRIGGER project_task_session_membership;

UPDATE agent_sessions SET wave_id=proposed.wave_id,agent_process_id=proposed.agent_process_id,transport_lf_process_id=proposed.transport_lf_process_id,task_id=proposed.task_id,id=proposed.id FROM canonical_agent_sessions proposed WHERE agent_sessions.rowid=proposed.source_rowid AND (agent_sessions.wave_id IS NOT proposed.wave_id OR agent_sessions.agent_process_id IS NOT proposed.agent_process_id OR agent_sessions.transport_lf_process_id IS NOT proposed.transport_lf_process_id OR agent_sessions.task_id IS NOT proposed.task_id OR agent_sessions.id IS NOT proposed.id);

UPDATE ci_incidents SET repair_lf_process_id=proposed.repair_lf_process_id,task_id=proposed.task_id,pr_id=proposed.pr_id,landing_id=proposed.landing_id,repair_session_id=proposed.repair_session_id FROM canonical_ci_incidents proposed WHERE ci_incidents.rowid=proposed.source_rowid AND (ci_incidents.repair_lf_process_id IS NOT proposed.repair_lf_process_id OR ci_incidents.task_id IS NOT proposed.task_id OR ci_incidents.pr_id IS NOT proposed.pr_id OR ci_incidents.landing_id IS NOT proposed.landing_id OR ci_incidents.repair_session_id IS NOT proposed.repair_session_id);

UPDATE flow_process_steps SET lf_process_id=proposed.lf_process_id,flow_lf_process_id=proposed.flow_lf_process_id FROM canonical_flow_process_steps proposed WHERE flow_process_steps.rowid=proposed.source_rowid AND (flow_process_steps.lf_process_id IS NOT proposed.lf_process_id OR flow_process_steps.flow_lf_process_id IS NOT proposed.flow_lf_process_id);

UPDATE flow_processes SET lf_process_id=proposed.lf_process_id FROM canonical_flow_processes proposed WHERE flow_processes.rowid=proposed.source_rowid AND (flow_processes.lf_process_id IS NOT proposed.lf_process_id);

UPDATE linear_team_checks SET team_id=proposed.team_id FROM canonical_linear_team_checks proposed WHERE linear_team_checks.rowid=proposed.source_rowid AND (linear_team_checks.team_id IS NOT proposed.team_id);

UPDATE machines SET id=proposed.id FROM canonical_machines proposed WHERE machines.rowid=proposed.source_rowid AND (machines.id IS NOT proposed.id);

UPDATE metric_instruments SET wave_id=proposed.wave_id FROM canonical_metric_instruments proposed WHERE metric_instruments.rowid=proposed.source_rowid AND (metric_instruments.wave_id IS NOT proposed.wave_id);

UPDATE metric_observations SET wave_id=proposed.wave_id FROM canonical_metric_observations proposed WHERE metric_observations.rowid=proposed.source_rowid AND (metric_observations.wave_id IS NOT proposed.wave_id);

UPDATE planning_peer_changes SET id=proposed.id,mutation=proposed.mutation FROM canonical_planning_peer_changes proposed WHERE planning_peer_changes.rowid=proposed.source_rowid AND (planning_peer_changes.id IS NOT proposed.id OR planning_peer_changes.mutation IS NOT proposed.mutation);

UPDATE planning_peer_conflicts SET object_id=proposed.object_id FROM canonical_planning_peer_conflicts proposed WHERE planning_peer_conflicts.rowid=proposed.source_rowid AND (planning_peer_conflicts.object_id IS NOT proposed.object_id);

UPDATE planning_peer_observed SET id=proposed.id FROM canonical_planning_peer_observed proposed WHERE planning_peer_observed.rowid=proposed.source_rowid AND (planning_peer_observed.id IS NOT proposed.id);

UPDATE planning_peer_routes SET mutation_id=proposed.mutation_id FROM canonical_planning_peer_routes proposed WHERE planning_peer_routes.rowid=proposed.source_rowid AND (planning_peer_routes.mutation_id IS NOT proposed.mutation_id);

UPDATE pm_issue_changes SET issue_id=proposed.issue_id FROM canonical_pm_issue_changes proposed WHERE pm_issue_changes.rowid=proposed.source_rowid AND (pm_issue_changes.issue_id IS NOT proposed.issue_id);

UPDATE pm_items SET id=proposed.id,project_id=proposed.project_id,team_id=proposed.team_id,assignee=proposed.assignee,claimed_project_id=proposed.claimed_project_id,identifier=proposed.identifier FROM canonical_pm_items proposed WHERE pm_items.rowid=proposed.source_rowid AND (pm_items.id IS NOT proposed.id OR pm_items.project_id IS NOT proposed.project_id OR pm_items.team_id IS NOT proposed.team_id OR pm_items.assignee IS NOT proposed.assignee OR pm_items.claimed_project_id IS NOT proposed.claimed_project_id OR pm_items.identifier IS NOT proposed.identifier);

UPDATE pm_project_name_cutover SET id=proposed.id,body=proposed.body FROM canonical_pm_project_name_cutover proposed WHERE pm_project_name_cutover.rowid=proposed.source_rowid AND (pm_project_name_cutover.id IS NOT proposed.id OR pm_project_name_cutover.body IS NOT proposed.body);

UPDATE pm_projects SET id=proposed.id,initiative_ids=proposed.initiative_ids,team_ids=proposed.team_ids,task_order_json=proposed.task_order_json FROM canonical_pm_projects proposed WHERE pm_projects.rowid=proposed.source_rowid AND (pm_projects.id IS NOT proposed.id OR pm_projects.initiative_ids IS NOT proposed.initiative_ids OR pm_projects.team_ids IS NOT proposed.team_ids OR pm_projects.task_order_json IS NOT proposed.task_order_json);

UPDATE pm_wave_projects SET wave_id=proposed.wave_id,project_id=proposed.project_id FROM canonical_pm_wave_projects proposed WHERE pm_wave_projects.rowid=proposed.source_rowid AND (pm_wave_projects.wave_id IS NOT proposed.wave_id OR pm_wave_projects.project_id IS NOT proposed.project_id);

UPDATE pm_wave_sync SET wave_id=proposed.wave_id,initiative=proposed.initiative FROM canonical_pm_wave_sync proposed WHERE pm_wave_sync.rowid=proposed.source_rowid AND (pm_wave_sync.wave_id IS NOT proposed.wave_id OR pm_wave_sync.initiative IS NOT proposed.initiative);

UPDATE pr_landings SET task_id=proposed.task_id,supervisor_machine_id=proposed.supervisor_machine_id,id=proposed.id,retired_supervisor_json=proposed.retired_supervisor_json FROM canonical_pr_landings proposed WHERE pr_landings.rowid=proposed.source_rowid AND (pr_landings.task_id IS NOT proposed.task_id OR pr_landings.supervisor_machine_id IS NOT proposed.supervisor_machine_id OR pr_landings.id IS NOT proposed.id OR pr_landings.retired_supervisor_json IS NOT proposed.retired_supervisor_json);

UPDATE processes SET id=proposed.id,trace_id=proposed.trace_id,parent_lf_process_id=proposed.parent_lf_process_id,caller_agent_process_id=proposed.caller_agent_process_id,attached_lf_process_id=proposed.attached_lf_process_id,attachment_token=proposed.attachment_token,caller_session_id=proposed.caller_session_id,agent_session_id=proposed.agent_session_id FROM canonical_processes proposed WHERE processes.rowid=proposed.source_rowid AND (processes.id IS NOT proposed.id OR processes.trace_id IS NOT proposed.trace_id OR processes.parent_lf_process_id IS NOT proposed.parent_lf_process_id OR processes.caller_agent_process_id IS NOT proposed.caller_agent_process_id OR processes.attached_lf_process_id IS NOT proposed.attached_lf_process_id OR processes.attachment_token IS NOT proposed.attachment_token OR processes.caller_session_id IS NOT proposed.caller_session_id OR processes.agent_session_id IS NOT proposed.agent_session_id);

UPDATE project_binding_imports SET wave_id=proposed.wave_id FROM canonical_project_binding_imports proposed WHERE project_binding_imports.rowid=proposed.source_rowid AND (project_binding_imports.wave_id IS NOT proposed.wave_id);

UPDATE project_changes SET id=proposed.id,project_id=proposed.project_id,value_json=proposed.value_json,base_json=proposed.base_json,conflict_json=proposed.conflict_json,order_effects_json=proposed.order_effects_json FROM canonical_project_changes proposed WHERE project_changes.rowid=proposed.source_rowid AND (project_changes.id IS NOT proposed.id OR project_changes.project_id IS NOT proposed.project_id OR project_changes.value_json IS NOT proposed.value_json OR project_changes.base_json IS NOT proposed.base_json OR project_changes.conflict_json IS NOT proposed.conflict_json OR project_changes.order_effects_json IS NOT proposed.order_effects_json);

UPDATE project_events SET project_id=proposed.project_id,kind_json=proposed.kind_json FROM canonical_project_events proposed WHERE project_events.rowid=proposed.source_rowid AND (project_events.project_id IS NOT proposed.project_id OR project_events.kind_json IS NOT proposed.kind_json);

UPDATE project_transition_items SET wave_id=proposed.wave_id,successor_id=proposed.successor_id,issue_id=proposed.issue_id FROM canonical_project_transition_items proposed WHERE project_transition_items.rowid=proposed.source_rowid AND (project_transition_items.wave_id IS NOT proposed.wave_id OR project_transition_items.successor_id IS NOT proposed.successor_id OR project_transition_items.issue_id IS NOT proposed.issue_id);

UPDATE project_transitions SET wave_id=proposed.wave_id,successor_id=proposed.successor_id,predecessor_id=proposed.predecessor_id,local_plan_json=proposed.local_plan_json,export_json=proposed.export_json FROM canonical_project_transitions proposed WHERE project_transitions.rowid=proposed.source_rowid AND (project_transitions.wave_id IS NOT proposed.wave_id OR project_transitions.successor_id IS NOT proposed.successor_id OR project_transitions.predecessor_id IS NOT proposed.predecessor_id OR project_transitions.local_plan_json IS NOT proposed.local_plan_json OR project_transitions.export_json IS NOT proposed.export_json);

UPDATE projects SET wave_id=proposed.wave_id,external_project_id=proposed.external_project_id,id=proposed.id,planning_initiatives=proposed.planning_initiatives,planning_teams=proposed.planning_teams FROM canonical_projects proposed WHERE projects.rowid=proposed.source_rowid AND (projects.wave_id IS NOT proposed.wave_id OR projects.external_project_id IS NOT proposed.external_project_id OR projects.id IS NOT proposed.id OR projects.planning_initiatives IS NOT proposed.planning_initiatives OR projects.planning_teams IS NOT proposed.planning_teams);

UPDATE relocated_task_processes SET process_id=proposed.process_id,task_id=proposed.task_id FROM canonical_relocated_task_processes proposed WHERE relocated_task_processes.rowid=proposed.source_rowid AND (relocated_task_processes.process_id IS NOT proposed.process_id OR relocated_task_processes.task_id IS NOT proposed.task_id);

UPDATE repository_plans SET id=proposed.id FROM canonical_repository_plans proposed WHERE repository_plans.rowid=proposed.source_rowid AND (repository_plans.id IS NOT proposed.id);

UPDATE session_activity SET attachment_token=proposed.attachment_token,agent_process_id=proposed.agent_process_id,session_id=proposed.session_id FROM canonical_session_activity proposed WHERE session_activity.rowid=proposed.source_rowid AND (session_activity.attachment_token IS NOT proposed.attachment_token OR session_activity.agent_process_id IS NOT proposed.agent_process_id OR session_activity.session_id IS NOT proposed.session_id);

UPDATE session_events SET lf_process_id=proposed.lf_process_id,wave_id=proposed.wave_id,agent_process_id=proposed.agent_process_id,task_id=proposed.task_id,session_id=proposed.session_id,payload=proposed.payload FROM canonical_session_events proposed WHERE session_events.rowid=proposed.source_rowid AND (session_events.lf_process_id IS NOT proposed.lf_process_id OR session_events.wave_id IS NOT proposed.wave_id OR session_events.agent_process_id IS NOT proposed.agent_process_id OR session_events.task_id IS NOT proposed.task_id OR session_events.session_id IS NOT proposed.session_id OR session_events.payload IS NOT proposed.payload);

UPDATE session_steers SET steer_id=proposed.steer_id,session_id=proposed.session_id FROM canonical_session_steers proposed WHERE session_steers.rowid=proposed.source_rowid AND (session_steers.steer_id IS NOT proposed.steer_id OR session_steers.session_id IS NOT proposed.session_id);

UPDATE steer_adapters SET agent_process_id=proposed.agent_process_id FROM canonical_steer_adapters proposed WHERE steer_adapters.rowid=proposed.source_rowid AND (steer_adapters.agent_process_id IS NOT proposed.agent_process_id);

UPDATE steer_batches SET id=proposed.id,agent_process_id=proposed.agent_process_id,attachment_token=proposed.attachment_token,session_id=proposed.session_id FROM canonical_steer_batches proposed WHERE steer_batches.rowid=proposed.source_rowid AND (steer_batches.id IS NOT proposed.id OR steer_batches.agent_process_id IS NOT proposed.agent_process_id OR steer_batches.attachment_token IS NOT proposed.attachment_token OR steer_batches.session_id IS NOT proposed.session_id);

UPDATE steer_deliveries SET steer_id=proposed.steer_id,batch_id=proposed.batch_id,session_id=proposed.session_id FROM canonical_steer_deliveries proposed WHERE steer_deliveries.rowid=proposed.source_rowid AND (steer_deliveries.steer_id IS NOT proposed.steer_id OR steer_deliveries.batch_id IS NOT proposed.batch_id OR steer_deliveries.session_id IS NOT proposed.session_id);

UPDATE steers SET id=proposed.id,wave_id=proposed.wave_id,task_id=proposed.task_id,project_id=proposed.project_id,session_id=proposed.session_id FROM canonical_steers proposed WHERE steers.rowid=proposed.source_rowid AND (steers.id IS NOT proposed.id OR steers.wave_id IS NOT proposed.wave_id OR steers.task_id IS NOT proposed.task_id OR steers.project_id IS NOT proposed.project_id OR steers.session_id IS NOT proposed.session_id);

UPDATE task_admissions SET task_id=proposed.task_id,machine_id=proposed.machine_id FROM canonical_task_admissions proposed WHERE task_admissions.rowid=proposed.source_rowid AND (task_admissions.task_id IS NOT proposed.task_id OR task_admissions.machine_id IS NOT proposed.machine_id);

UPDATE task_changes SET id=proposed.id,task_id=proposed.task_id,value_json=proposed.value_json,base_json=proposed.base_json,conflict_json=proposed.conflict_json FROM canonical_task_changes proposed WHERE task_changes.rowid=proposed.source_rowid AND (task_changes.id IS NOT proposed.id OR task_changes.task_id IS NOT proposed.task_id OR task_changes.value_json IS NOT proposed.value_json OR task_changes.base_json IS NOT proposed.base_json OR task_changes.conflict_json IS NOT proposed.conflict_json);

UPDATE task_comment_deliveries SET comment_id=proposed.comment_id,comment_json=proposed.comment_json,conflicting_comment_json=proposed.conflicting_comment_json FROM canonical_task_comment_deliveries proposed WHERE task_comment_deliveries.rowid=proposed.source_rowid AND (task_comment_deliveries.comment_id IS NOT proposed.comment_id OR task_comment_deliveries.comment_json IS NOT proposed.comment_json OR task_comment_deliveries.conflicting_comment_json IS NOT proposed.conflicting_comment_json);

UPDATE task_comments SET id=proposed.id,task_id=proposed.task_id FROM canonical_task_comments proposed WHERE task_comments.rowid=proposed.source_rowid AND (task_comments.id IS NOT proposed.id OR task_comments.task_id IS NOT proposed.task_id);

UPDATE task_creation_intents SET task_id=proposed.task_id,project_id=proposed.project_id,repository_id=proposed.repository_id,export_json=proposed.export_json FROM canonical_task_creation_intents proposed WHERE task_creation_intents.rowid=proposed.source_rowid AND (task_creation_intents.repository_id IS NOT proposed.repository_id OR task_creation_intents.task_id IS NOT proposed.task_id OR task_creation_intents.project_id IS NOT proposed.project_id OR task_creation_intents.export_json IS NOT proposed.export_json);

UPDATE task_deletions SET wave_id=proposed.wave_id,issue_id=proposed.issue_id,identifier=proposed.identifier FROM canonical_task_deletions proposed WHERE task_deletions.rowid=proposed.source_rowid AND (task_deletions.wave_id IS NOT proposed.wave_id OR task_deletions.issue_id IS NOT proposed.issue_id OR task_deletions.identifier IS NOT proposed.identifier);

UPDATE task_events SET task_id=proposed.task_id,kind_json=proposed.kind_json FROM canonical_task_events proposed WHERE task_events.rowid=proposed.source_rowid AND (task_events.task_id IS NOT proposed.task_id OR task_events.kind_json IS NOT proposed.kind_json);

UPDATE task_issue_identities SET wave_id=proposed.wave_id,issue_id=proposed.issue_id,identifier=proposed.identifier FROM canonical_task_issue_identities proposed WHERE task_issue_identities.rowid=proposed.source_rowid AND (task_issue_identities.wave_id IS NOT proposed.wave_id OR task_issue_identities.issue_id IS NOT proposed.issue_id OR task_issue_identities.identifier IS NOT proposed.identifier);

UPDATE task_linear_ingested_comments SET comment_id=proposed.comment_id,task_id=proposed.task_id FROM canonical_task_linear_ingested_comments proposed WHERE task_linear_ingested_comments.rowid=proposed.source_rowid AND (task_linear_ingested_comments.comment_id IS NOT proposed.comment_id OR task_linear_ingested_comments.task_id IS NOT proposed.task_id);

UPDATE task_linear_observations SET task_id=proposed.task_id FROM canonical_task_linear_observations proposed WHERE task_linear_observations.rowid=proposed.source_rowid AND (task_linear_observations.task_id IS NOT proposed.task_id);

UPDATE task_pr_repair_incidents SET task_pr_id=proposed.task_pr_id FROM canonical_task_pr_repair_incidents proposed WHERE task_pr_repair_incidents.rowid=proposed.source_rowid AND (task_pr_repair_incidents.task_pr_id IS NOT proposed.task_pr_id);

UPDATE task_prs SET linear_attachment_id=proposed.linear_attachment_id,linear_comment_id=proposed.linear_comment_id,task_id=proposed.task_id,id=proposed.id,parent_pr_id=proposed.parent_pr_id FROM canonical_task_prs proposed WHERE task_prs.rowid=proposed.source_rowid AND (task_prs.linear_attachment_id IS NOT proposed.linear_attachment_id OR task_prs.linear_comment_id IS NOT proposed.linear_comment_id OR task_prs.task_id IS NOT proposed.task_id OR task_prs.id IS NOT proposed.id OR task_prs.parent_pr_id IS NOT proposed.parent_pr_id);

UPDATE task_state_deliveries SET id=proposed.id,task_id=proposed.task_id FROM canonical_task_state_deliveries proposed WHERE task_state_deliveries.rowid=proposed.source_rowid AND (task_state_deliveries.id IS NOT proposed.id OR task_state_deliveries.task_id IS NOT proposed.task_id);

UPDATE task_workflow_moves SET lf_process_id=proposed.lf_process_id,task_id=proposed.task_id FROM canonical_task_workflow_moves proposed WHERE task_workflow_moves.rowid=proposed.source_rowid AND (task_workflow_moves.lf_process_id IS NOT proposed.lf_process_id OR task_workflow_moves.task_id IS NOT proposed.task_id);

UPDATE task_workflows SET lf_process_id=proposed.lf_process_id,task_id=proposed.task_id FROM canonical_task_workflows proposed WHERE task_workflows.rowid=proposed.source_rowid AND (task_workflows.lf_process_id IS NOT proposed.lf_process_id OR task_workflows.task_id IS NOT proposed.task_id);

UPDATE tasks SET external_issue_id=proposed.external_issue_id,planning_assignee=proposed.planning_assignee,planning_team_id=proposed.planning_team_id,id=proposed.id,project_id=proposed.project_id,checkout_machine_id=proposed.checkout_machine_id,parent_pr_id=proposed.parent_pr_id,primary_session_id=proposed.primary_session_id,issue_identifier=proposed.issue_identifier,admission_authority=proposed.admission_authority,repository_id=proposed.repository_id FROM canonical_tasks proposed WHERE tasks.rowid=proposed.source_rowid AND (tasks.repository_id IS NOT proposed.repository_id OR tasks.external_issue_id IS NOT proposed.external_issue_id OR tasks.planning_assignee IS NOT proposed.planning_assignee OR tasks.planning_team_id IS NOT proposed.planning_team_id OR tasks.id IS NOT proposed.id OR tasks.project_id IS NOT proposed.project_id OR tasks.checkout_machine_id IS NOT proposed.checkout_machine_id OR tasks.parent_pr_id IS NOT proposed.parent_pr_id OR tasks.primary_session_id IS NOT proposed.primary_session_id OR tasks.issue_identifier IS NOT proposed.issue_identifier OR tasks.admission_authority IS NOT proposed.admission_authority);

UPDATE tool_responses SET id=proposed.id,work_id=proposed.work_id FROM canonical_tool_responses proposed WHERE tool_responses.rowid=proposed.source_rowid AND (tool_responses.id IS NOT proposed.id OR tool_responses.work_id IS NOT proposed.work_id);

UPDATE wave_workflows SET wave_id=proposed.wave_id FROM canonical_wave_workflows proposed WHERE wave_workflows.rowid=proposed.source_rowid AND (wave_workflows.wave_id IS NOT proposed.wave_id);

UPDATE waves SET id=proposed.id,parent_wave_id=proposed.parent_wave_id,superseded_by_wave_id=proposed.superseded_by_wave_id,project_activation_lf_process_id=proposed.project_activation_lf_process_id,current_project_id=proposed.current_project_id FROM canonical_waves proposed WHERE waves.rowid=proposed.source_rowid AND (waves.id IS NOT proposed.id OR waves.parent_wave_id IS NOT proposed.parent_wave_id OR waves.superseded_by_wave_id IS NOT proposed.superseded_by_wave_id OR waves.project_activation_lf_process_id IS NOT proposed.project_activation_lf_process_id OR waves.current_project_id IS NOT proposed.current_project_id);

UPDATE work_placements SET wave_id=proposed.wave_id,task_id=proposed.task_id,project_id=proposed.project_id,machine_id=proposed.machine_id FROM canonical_work_placements proposed WHERE work_placements.rowid=proposed.source_rowid AND (work_placements.wave_id IS NOT proposed.wave_id OR work_placements.task_id IS NOT proposed.task_id OR work_placements.project_id IS NOT proposed.project_id OR work_placements.machine_id IS NOT proposed.machine_id);

CREATE TRIGGER task_pr_repair_incidents_are_immutable
BEFORE UPDATE ON task_pr_repair_incidents
BEGIN
    SELECT RAISE(ABORT, 'Task PR repair incidents are immutable');
END;

CREATE TRIGGER preserve_capture_update BEFORE UPDATE ON session_events WHEN OLD.kind='captured'
BEGIN SELECT RAISE(ABORT,'Captured Session evidence is immutable'); END;

CREATE TRIGGER validate_session_capture BEFORE UPDATE OF current_capture ON agent_sessions
WHEN NEW.current_capture IS NOT NULL AND NOT EXISTS(SELECT 1 FROM session_events WHERE seq=NEW.current_capture AND session_id=NEW.id AND kind='captured')
BEGIN SELECT RAISE(ABORT,'Captured event belongs to another Session'); END;

CREATE TRIGGER task_first_conversation_bind AFTER UPDATE OF task_id ON agent_sessions
WHEN OLD.task_id IS NULL AND NEW.task_id IS NOT NULL BEGIN
    UPDATE tasks SET started_at=coalesce(NEW.bound_at,CAST(strftime('%s','now') AS INTEGER))
    WHERE id=NEW.task_id AND started_at IS NULL;
END;

CREATE TRIGGER selected_project_wave_update BEFORE UPDATE OF current_project_id ON waves
WHEN NEW.current_project_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM projects WHERE id=NEW.current_project_id AND wave_id=NEW.id
) BEGIN SELECT RAISE(ABORT, 'selected Project belongs to another Wave'); END;

CREATE TRIGGER validate_conversation_ancestry_update BEFORE UPDATE OF task_id,wave_id ON agent_sessions BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM tasks t LEFT JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id IS NEW.wave_id
    ) THEN RAISE(ABORT,'Session Task and Wave disagree') END;
    SELECT CASE WHEN OLD.task_id IS NOT NULL AND NEW.task_id IS NOT OLD.task_id
        THEN RAISE(ABORT,'Session Task binding is permanent') END;
    SELECT CASE WHEN OLD.task_id IS NULL AND OLD.wave_id IS NOT NULL AND NEW.wave_id IS NOT OLD.wave_id
        THEN RAISE(ABORT,'Wave Session binding is permanent') END;
END;

CREATE TRIGGER task_workflow_graph_is_fixed BEFORE UPDATE OF graph ON task_workflows BEGIN
    SELECT RAISE(ABORT,'A Workflow graph is fixed when taken up');
END;

CREATE TRIGGER task_workflow_moves_are_append_only BEFORE UPDATE ON task_workflow_moves BEGIN
    SELECT RAISE(ABORT,'Workflow history is append-only');
END;

CREATE TRIGGER flow_processes_are_append_only BEFORE UPDATE ON flow_processes BEGIN
    SELECT RAISE(ABORT,'A Flow record is append-only');
END;

CREATE TRIGGER flow_process_steps_are_append_only BEFORE UPDATE ON flow_process_steps BEGIN
    SELECT RAISE(ABORT,'A Flow record is append-only');
END;

CREATE TRIGGER pr_landings_no_machine_update
BEFORE UPDATE OF supervisor_placement ON pr_landings WHEN NEW.supervisor_placement='home'
BEGIN
    SELECT RAISE(ABORT, 'Remote machine landing supervision is retired');
END;

CREATE TRIGGER selected_project_owner_update BEFORE UPDATE OF wave_id ON projects
WHEN EXISTS(SELECT 1 FROM waves WHERE current_project_id=NEW.id AND id!=NEW.wave_id)
BEGIN SELECT RAISE(ABORT, 'selected Project cannot change Wave ownership'); END;

CREATE TRIGGER validate_task_started_update BEFORE UPDATE OF started_at ON tasks BEGIN
    SELECT CASE WHEN OLD.started_at IS NOT NULL AND NEW.started_at IS NOT OLD.started_at
        THEN RAISE(ABORT,'Task first assignment time cannot change') END;
    SELECT CASE WHEN NEW.started_at IS NOT NULL AND NOT (
        EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM flow_processes f JOIN processes driver ON driver.id=f.lf_process_id WHERE NEW.worktree!=''
            AND (driver.cwd=rtrim(NEW.worktree,'/') OR instr(driver.cwd,rtrim(NEW.worktree,'/')||'/')=1)) OR
        EXISTS(SELECT 1 FROM task_events e WHERE e.task_id=NEW.id
            AND json_extract(e.kind_json,'$.kind')='started')
    ) THEN RAISE(ABORT,'Started requires recorded Task work') END;
END;

CREATE TRIGGER task_prs_preserve_merged_at
BEFORE UPDATE OF merged_at ON task_prs
WHEN OLD.merged_at IS NOT NULL
 AND NEW.merged_at IS NOT OLD.merged_at
BEGIN
    SELECT RAISE(ABORT, 'Task PR merge evidence is immutable');
END;

CREATE TRIGGER task_pr_history_immutable BEFORE UPDATE ON task_prs WHEN OLD.historical=1
BEGIN SELECT RAISE(ABORT, 'historical Task PRs are read-only'); END;

CREATE TRIGGER task_pr_placement_update BEFORE UPDATE OF branch,base_commit,parent_pr_id ON task_prs
WHEN NEW.historical=0 AND NOT EXISTS(SELECT 1 FROM tasks t WHERE t.id=NEW.task_id
 AND t.branch=NEW.branch AND t.base_commit=NEW.base_commit AND t.parent_pr_id IS NEW.parent_pr_id)
BEGIN SELECT RAISE(ABORT, 'Task PR placement differs from Task'); END;

CREATE TRIGGER task_admissions_immutable BEFORE UPDATE ON task_admissions
BEGIN SELECT RAISE(ABORT, 'Task admission cannot transfer execution'); END;

CREATE TRIGGER task_pr_history_no_mark BEFORE UPDATE OF historical ON task_prs
WHEN NEW.historical<>OLD.historical AND NOT (
 OLD.historical=0 AND NEW.historical=1
 AND (OLD.merge_commit IS NOT NULL OR OLD.abandoned_at IS NOT NULL))
BEGIN SELECT RAISE(ABORT, 'only settled Task PRs can become history'); END;

CREATE TRIGGER task_placement_to_pr AFTER UPDATE OF branch,base_commit,parent_pr_id ON tasks
BEGIN
 UPDATE task_prs SET branch=NEW.branch, base_commit=NEW.base_commit,
 parent_pr_id=NEW.parent_pr_id WHERE task_id=NEW.id AND historical=0
 AND merge_commit IS NULL AND abandoned_at IS NULL;
END;

CREATE TRIGGER task_repository_is_fixed BEFORE UPDATE OF repository_id ON tasks
WHEN NEW.repository_id IS NOT OLD.repository_id
BEGIN SELECT RAISE(ABORT, 'Task repository identity cannot change'); END;

CREATE TRIGGER validate_task_repository_update BEFORE UPDATE OF project_id,repository_id ON tasks
WHEN NEW.project_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM projects p JOIN waves w ON w.id=p.wave_id
    JOIN repository_plans r ON r.repo=w.repo WHERE p.id=NEW.project_id AND r.id=NEW.repository_id
)
BEGIN SELECT RAISE(ABORT, 'Task Project must belong to its repository'); END;

CREATE TRIGGER validate_project_task_repositories BEFORE UPDATE OF wave_id ON projects
WHEN EXISTS (SELECT 1 FROM tasks t JOIN repository_plans r ON r.id=t.repository_id
    WHERE t.project_id=NEW.id AND NOT EXISTS(SELECT 1 FROM waves w WHERE w.id=NEW.wave_id AND w.repo=r.repo))
BEGIN SELECT RAISE(ABORT, 'Project move would change Task repository'); END;

CREATE TRIGGER task_session_membership AFTER UPDATE OF project_id ON tasks BEGIN
    UPDATE agent_sessions SET wave_id=(SELECT wave_id FROM projects WHERE id=NEW.project_id) WHERE task_id=NEW.id;
END;

CREATE TRIGGER project_task_session_membership AFTER UPDATE OF wave_id ON projects BEGIN
    UPDATE agent_sessions SET wave_id=NEW.wave_id WHERE task_id IN (SELECT id FROM tasks WHERE project_id=NEW.id);
END;

DROP TABLE canonical_agent_sessions;

DROP TABLE canonical_ci_incidents;

DROP TABLE canonical_flow_process_steps;

DROP TABLE canonical_flow_processes;

DROP TABLE canonical_linear_team_checks;

DROP TABLE canonical_machines;

DROP TABLE canonical_metric_instruments;

DROP TABLE canonical_metric_observations;

DROP TABLE canonical_planning_peer_changes;

DROP TABLE canonical_planning_peer_conflicts;

DROP TABLE canonical_planning_peer_observed;

DROP TABLE canonical_planning_peer_routes;

DROP TABLE canonical_pm_issue_changes;

DROP TABLE canonical_pm_items;

DROP TABLE canonical_pm_project_name_cutover;

DROP TABLE canonical_pm_projects;

DROP TABLE canonical_pm_wave_projects;

DROP TABLE canonical_pm_wave_sync;

DROP TABLE canonical_pr_landings;

DROP TABLE canonical_processes;

DROP TABLE canonical_project_binding_imports;

DROP TABLE canonical_project_changes;

DROP TABLE canonical_project_events;

DROP TABLE canonical_project_transition_items;

DROP TABLE canonical_project_transitions;

DROP TABLE canonical_projects;

DROP TABLE canonical_relocated_task_processes;

DROP TABLE canonical_repository_plans;

DROP TABLE canonical_session_activity;

DROP TABLE canonical_session_events;

DROP TABLE canonical_session_steers;

DROP TABLE canonical_steer_adapters;

DROP TABLE canonical_steer_batches;

DROP TABLE canonical_steer_deliveries;

DROP TABLE canonical_steers;

DROP TABLE canonical_task_admissions;

DROP TABLE canonical_task_changes;

DROP TABLE canonical_task_comment_deliveries;

DROP TABLE canonical_task_comments;

DROP TABLE canonical_task_creation_intents;

DROP TABLE canonical_task_deletions;

DROP TABLE canonical_task_events;

DROP TABLE canonical_task_issue_identities;

DROP TABLE canonical_task_linear_ingested_comments;

DROP TABLE canonical_task_linear_observations;

DROP TABLE canonical_task_pr_repair_incidents;

DROP TABLE canonical_task_prs;

DROP TABLE canonical_task_state_deliveries;

DROP TABLE canonical_task_workflow_moves;

DROP TABLE canonical_task_workflows;

DROP TABLE canonical_tasks;

DROP TABLE canonical_tool_responses;

DROP TABLE canonical_wave_workflows;

DROP TABLE canonical_waves;

DROP TABLE canonical_work_placements;

DROP TABLE canonical_json;
DROP TABLE canonical_values;
DROP TRIGGER canonical_identity_conflict;
DROP TABLE canonical_identity_conflicts;

-- Future writes must arrive through their domain constructors. Locators/native payloads are excluded.

CREATE TRIGGER canonical_agent_sessions_insert BEFORE INSERT ON agent_sessions
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: agent_sessions') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.transport_lf_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_agent_sessions_update BEFORE UPDATE OF wave_id,agent_process_id,transport_lf_process_id,task_id,id ON agent_sessions
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: agent_sessions') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.transport_lf_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_ci_incidents_insert BEFORE INSERT ON ci_incidents
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: ci_incidents') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.repair_lf_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.pr_id AS original
UNION ALL
SELECT 'landing_' AS domain,NEW.landing_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.repair_session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_ci_incidents_update BEFORE UPDATE OF repair_lf_process_id,task_id,pr_id,landing_id,repair_session_id ON ci_incidents
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: ci_incidents') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.repair_lf_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.pr_id AS original
UNION ALL
SELECT 'landing_' AS domain,NEW.landing_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.repair_session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_flow_process_steps_insert BEFORE INSERT ON flow_process_steps
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: flow_process_steps') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.flow_lf_process_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_flow_process_steps_update BEFORE UPDATE OF lf_process_id,flow_lf_process_id ON flow_process_steps
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: flow_process_steps') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.flow_lf_process_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_flow_processes_insert BEFORE INSERT ON flow_processes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: flow_processes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.lf_process_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_flow_processes_update BEFORE UPDATE OF lf_process_id ON flow_processes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: flow_processes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.lf_process_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_linear_team_checks_insert BEFORE INSERT ON linear_team_checks
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: linear_team_checks') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.team_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_linear_team_checks_update BEFORE UPDATE OF team_id ON linear_team_checks
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: linear_team_checks') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.team_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_machines_insert BEFORE INSERT ON machines
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: machines') FROM (
 WITH incoming AS (
SELECT 'home_' AS domain,NEW.id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_machines_update BEFORE UPDATE OF id ON machines
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: machines') FROM (
 WITH incoming AS (
SELECT 'home_' AS domain,NEW.id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_metric_instruments_insert BEFORE INSERT ON metric_instruments
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: metric_instruments') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_metric_instruments_update BEFORE UPDATE OF wave_id ON metric_instruments
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: metric_instruments') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_metric_observations_insert BEFORE INSERT ON metric_observations
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: metric_observations') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_metric_observations_update BEFORE UPDATE OF wave_id ON metric_observations
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: metric_observations') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_planning_peer_changes_insert BEFORE INSERT ON planning_peer_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: planning_peer_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN path='$.edit.target' AND NEW.record_table='waves' THEN 'uuid' WHEN path='$.edit.target' AND NEW.record_table='projects' THEN 'proj_' WHEN path='$.edit.target' AND NEW.record_table='tasks' THEN 'task_' WHEN path='$.edit.target' AND NEW.record_table='task_comments' THEN 'uuid' WHEN parent='$.parents' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('comment_task') THEN 'task_' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('task_identifier') THEN 'label' WHEN parent='$.edit.change.value' AND typeof(key)='integer' AND json_extract(doc,'$.edit.change.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.edit.change.value.machine_id' AND json_extract(doc,'$.edit.change.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.edit.change.value.repository_id' AND json_extract(doc,'$.edit.change.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.mutation AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.mutation) THEN NEW.mutation END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_planning_peer_changes_update BEFORE UPDATE OF id,mutation ON planning_peer_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: planning_peer_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN path='$.edit.target' AND NEW.record_table='waves' THEN 'uuid' WHEN path='$.edit.target' AND NEW.record_table='projects' THEN 'proj_' WHEN path='$.edit.target' AND NEW.record_table='tasks' THEN 'task_' WHEN path='$.edit.target' AND NEW.record_table='task_comments' THEN 'uuid' WHEN parent='$.parents' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('comment_task') THEN 'task_' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('task_identifier') THEN 'label' WHEN parent='$.edit.change.value' AND typeof(key)='integer' AND json_extract(doc,'$.edit.change.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.edit.change.value.machine_id' AND json_extract(doc,'$.edit.change.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.edit.change.value.repository_id' AND json_extract(doc,'$.edit.change.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.mutation AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.mutation) THEN NEW.mutation END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_planning_peer_conflicts_insert BEFORE INSERT ON planning_peer_conflicts
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: planning_peer_conflicts') FROM (
 WITH incoming AS (
SELECT CASE NEW.kind WHEN 'waves' THEN 'uuid' WHEN 'projects' THEN 'proj_' WHEN 'tasks' THEN 'task_' WHEN 'task_comments' THEN 'uuid' END AS domain,NEW.object_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_planning_peer_conflicts_update BEFORE UPDATE OF object_id ON planning_peer_conflicts
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: planning_peer_conflicts') FROM (
 WITH incoming AS (
SELECT CASE NEW.kind WHEN 'waves' THEN 'uuid' WHEN 'projects' THEN 'proj_' WHEN 'tasks' THEN 'task_' WHEN 'task_comments' THEN 'uuid' END AS domain,NEW.object_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_planning_peer_observed_insert BEFORE INSERT ON planning_peer_observed
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: planning_peer_observed') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_planning_peer_observed_update BEFORE UPDATE OF id ON planning_peer_observed
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: planning_peer_observed') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_planning_peer_routes_insert BEFORE INSERT ON planning_peer_routes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: planning_peer_routes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.mutation_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_planning_peer_routes_update BEFORE UPDATE OF mutation_id ON planning_peer_routes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: planning_peer_routes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.mutation_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_pm_issue_changes_insert BEFORE INSERT ON pm_issue_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_issue_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.issue_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_pm_issue_changes_update BEFORE UPDATE OF issue_id ON pm_issue_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_issue_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.issue_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_pm_items_insert BEFORE INSERT ON pm_items
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_items') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.team_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.assignee AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.claimed_project_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.identifier AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_pm_items_update BEFORE UPDATE OF id,project_id,team_id,assignee,claimed_project_id,identifier ON pm_items
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_items') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.team_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.assignee AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.claimed_project_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.identifier AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_pm_project_name_cutover_insert BEFORE INSERT ON pm_project_name_cutover
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_project_name_cutover') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' WHEN parent='$.initiative_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.team_ids' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.body AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.body) THEN NEW.body END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_pm_project_name_cutover_update BEFORE UPDATE OF id,body ON pm_project_name_cutover
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_project_name_cutover') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' WHEN parent='$.initiative_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.team_ids' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.body AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.body) THEN NEW.body END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_pm_projects_insert BEFORE INSERT ON pm_projects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_projects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.initiative_ids AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.initiative_ids) THEN NEW.initiative_ids END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.team_ids AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.team_ids) THEN NEW.team_ids END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.task_order_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.task_order_json) THEN NEW.task_order_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_pm_projects_update BEFORE UPDATE OF id,initiative_ids,team_ids,task_order_json ON pm_projects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_projects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.initiative_ids AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.initiative_ids) THEN NEW.initiative_ids END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.team_ids AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.team_ids) THEN NEW.team_ids END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.task_order_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.task_order_json) THEN NEW.task_order_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_pm_wave_projects_insert BEFORE INSERT ON pm_wave_projects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_wave_projects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_pm_wave_projects_update BEFORE UPDATE OF wave_id,project_id ON pm_wave_projects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_wave_projects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_pm_wave_sync_insert BEFORE INSERT ON pm_wave_sync
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_wave_sync') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.initiative AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_pm_wave_sync_update BEFORE UPDATE OF wave_id,initiative ON pm_wave_sync
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_wave_sync') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.initiative AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_pr_landings_insert BEFORE INSERT ON pr_landings
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pr_landings') FROM (
 WITH incoming AS (
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'home_' AS domain,NEW.supervisor_machine_id AS original
UNION ALL
SELECT 'landing_' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN path='$.home_id' THEN 'home_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.retired_supervisor_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.retired_supervisor_json) THEN NEW.retired_supervisor_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_pr_landings_update BEFORE UPDATE OF task_id,supervisor_machine_id,id,retired_supervisor_json ON pr_landings
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pr_landings') FROM (
 WITH incoming AS (
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'home_' AS domain,NEW.supervisor_machine_id AS original
UNION ALL
SELECT 'landing_' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN path='$.home_id' THEN 'home_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.retired_supervisor_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.retired_supervisor_json) THEN NEW.retired_supervisor_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_processes_insert BEFORE INSERT ON processes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: processes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.trace_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.parent_lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.caller_agent_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.attached_lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.attachment_token AS original
UNION ALL
SELECT 'session' AS domain,NEW.caller_session_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.agent_session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_processes_update BEFORE UPDATE OF id,trace_id,parent_lf_process_id,caller_agent_process_id,attached_lf_process_id,attachment_token,caller_session_id,agent_session_id ON processes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: processes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.trace_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.parent_lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.caller_agent_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.attached_lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.attachment_token AS original
UNION ALL
SELECT 'session' AS domain,NEW.caller_session_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.agent_session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_project_binding_imports_insert BEFORE INSERT ON project_binding_imports
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_binding_imports') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_project_binding_imports_update BEFORE UPDATE OF wave_id ON project_binding_imports
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_binding_imports') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_project_changes_insert BEFORE INSERT ON project_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT CASE WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('comment_task') THEN 'task_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value' AND typeof(key)='integer' AND json_extract(doc,'$.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.machine_id' AND json_extract(doc,'$.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.repository_id' AND json_extract(doc,'$.field')='task_admission_authority' THEN 'repo_' WHEN parent='$' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.value_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.value_json) THEN NEW.value_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' WHEN parent='$.value' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.base_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.base_json) THEN NEW.base_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' WHEN parent='$.value' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.conflict_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.conflict_json) THEN NEW.conflict_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN (parent GLOB '$[[]*].before' OR parent GLOB '$[[]*].after') AND typeof(key)='integer' THEN 'task_' WHEN parent GLOB '$[[]*]' AND key='issue' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.order_effects_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.order_effects_json) THEN NEW.order_effects_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_project_changes_update BEFORE UPDATE OF id,project_id,value_json,base_json,conflict_json,order_effects_json ON project_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT CASE WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('comment_task') THEN 'task_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value' AND typeof(key)='integer' AND json_extract(doc,'$.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.machine_id' AND json_extract(doc,'$.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.repository_id' AND json_extract(doc,'$.field')='task_admission_authority' THEN 'repo_' WHEN parent='$' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.value_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.value_json) THEN NEW.value_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' WHEN parent='$.value' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.base_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.base_json) THEN NEW.base_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' WHEN parent='$.value' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.conflict_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.conflict_json) THEN NEW.conflict_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN (parent GLOB '$[[]*].before' OR parent GLOB '$[[]*].after') AND typeof(key)='integer' THEN 'task_' WHEN parent GLOB '$[[]*]' AND key='issue' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.order_effects_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.order_effects_json) THEN NEW.order_effects_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_project_events_insert BEFORE INSERT ON project_events
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_events') FROM (
 WITH incoming AS (
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT CASE WHEN path='$.event.pr_id' THEN 'pr_' WHEN path='$.event.steer_id' THEN 'uuid' WHEN path='$.event.invocation_id' THEN 'uuid' WHEN path='$.event.relation_id' THEN 'uuid' WHEN path='$.event.source_issue_id' THEN 'uuid' WHEN path='$.event.target_issue_id' THEN 'uuid' WHEN parent='$.event.lf_process_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.event.exec_ids' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.event.intent.issue_id' THEN 'task_or_uuid' WHEN path='$.event.intent.project_id' THEN 'project_or_uuid' WHEN path='$.event.intent.relation_id' THEN 'uuid' WHEN path='$.event.intent.team_id' THEN 'uuid' WHEN path='$.event.intent.state_id' THEN 'uuid' WHEN path='$.event.intent.identifier' THEN 'label' WHEN path='$.event.link.issue_id' THEN 'task_or_uuid' WHEN path='$.event.link.project_id' THEN 'project_or_uuid' WHEN path='$.event.link.relation_id' THEN 'uuid' WHEN path='$.event.link.team_id' THEN 'uuid' WHEN path='$.event.link.state_id' THEN 'uuid' WHEN path='$.event.link.identifier' THEN 'label' WHEN path='$.task_id' THEN 'task_' WHEN path='$.steer_id' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.kind_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.kind_json) THEN NEW.kind_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_project_events_update BEFORE UPDATE OF project_id,kind_json ON project_events
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_events') FROM (
 WITH incoming AS (
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT CASE WHEN path='$.event.pr_id' THEN 'pr_' WHEN path='$.event.steer_id' THEN 'uuid' WHEN path='$.event.invocation_id' THEN 'uuid' WHEN path='$.event.relation_id' THEN 'uuid' WHEN path='$.event.source_issue_id' THEN 'uuid' WHEN path='$.event.target_issue_id' THEN 'uuid' WHEN parent='$.event.lf_process_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.event.exec_ids' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.event.intent.issue_id' THEN 'task_or_uuid' WHEN path='$.event.intent.project_id' THEN 'project_or_uuid' WHEN path='$.event.intent.relation_id' THEN 'uuid' WHEN path='$.event.intent.team_id' THEN 'uuid' WHEN path='$.event.intent.state_id' THEN 'uuid' WHEN path='$.event.intent.identifier' THEN 'label' WHEN path='$.event.link.issue_id' THEN 'task_or_uuid' WHEN path='$.event.link.project_id' THEN 'project_or_uuid' WHEN path='$.event.link.relation_id' THEN 'uuid' WHEN path='$.event.link.team_id' THEN 'uuid' WHEN path='$.event.link.state_id' THEN 'uuid' WHEN path='$.event.link.identifier' THEN 'label' WHEN path='$.task_id' THEN 'task_' WHEN path='$.steer_id' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.kind_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.kind_json) THEN NEW.kind_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_project_transition_items_insert BEFORE INSERT ON project_transition_items
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_transition_items') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'project_or_uuid' AS domain,NEW.successor_id AS original
UNION ALL
SELECT 'task_or_uuid' AS domain,NEW.issue_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_project_transition_items_update BEFORE UPDATE OF wave_id,successor_id,issue_id ON project_transition_items
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_transition_items') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'project_or_uuid' AS domain,NEW.successor_id AS original
UNION ALL
SELECT 'task_or_uuid' AS domain,NEW.issue_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_project_transitions_insert BEFORE INSERT ON project_transitions
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_transitions') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'project_or_uuid' AS domain,NEW.successor_id AS original
UNION ALL
SELECT 'project_or_uuid' AS domain,NEW.predecessor_id AS original
UNION ALL
SELECT CASE WHEN path='$.wave_id' THEN 'uuid' WHEN path='$.successor_id' THEN 'project_or_uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.local_plan_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.local_plan_json) THEN NEW.local_plan_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' WHEN path='$.initiative' THEN 'uuid' WHEN path='$.link_id' THEN 'uuid' WHEN path='$.input.id' THEN 'uuid' WHEN path='$.input.teamId' THEN 'uuid' WHEN path='$.input.projectId' THEN 'uuid' WHEN path='$.input.assigneeId' THEN 'uuid' WHEN parent='$.input.teamIds' AND typeof(key)='integer' THEN 'uuid' WHEN (parent GLOB '$.baseline[[]*].value') AND typeof(key)='integer' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*].value' AND key='machine_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN parent GLOB '$.baseline[[]*].value' AND key='repository_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field')='task_admission_authority' THEN 'repo_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_project','task_project') THEN 'proj_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('comment_task') THEN 'task_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('task_identifier') THEN 'label' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.export_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.export_json) THEN NEW.export_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_project_transitions_update BEFORE UPDATE OF wave_id,successor_id,predecessor_id,local_plan_json,export_json ON project_transitions
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_transitions') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'project_or_uuid' AS domain,NEW.successor_id AS original
UNION ALL
SELECT 'project_or_uuid' AS domain,NEW.predecessor_id AS original
UNION ALL
SELECT CASE WHEN path='$.wave_id' THEN 'uuid' WHEN path='$.successor_id' THEN 'project_or_uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.local_plan_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.local_plan_json) THEN NEW.local_plan_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' WHEN path='$.initiative' THEN 'uuid' WHEN path='$.link_id' THEN 'uuid' WHEN path='$.input.id' THEN 'uuid' WHEN path='$.input.teamId' THEN 'uuid' WHEN path='$.input.projectId' THEN 'uuid' WHEN path='$.input.assigneeId' THEN 'uuid' WHEN parent='$.input.teamIds' AND typeof(key)='integer' THEN 'uuid' WHEN (parent GLOB '$.baseline[[]*].value') AND typeof(key)='integer' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*].value' AND key='machine_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN parent GLOB '$.baseline[[]*].value' AND key='repository_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field')='task_admission_authority' THEN 'repo_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_project','task_project') THEN 'proj_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('comment_task') THEN 'task_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('task_identifier') THEN 'label' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.export_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.export_json) THEN NEW.export_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_projects_insert BEFORE INSERT ON projects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: projects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.external_project_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.planning_initiatives AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.planning_initiatives) THEN NEW.planning_initiatives END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.planning_teams AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.planning_teams) THEN NEW.planning_teams END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_projects_update BEFORE UPDATE OF wave_id,external_project_id,id,planning_initiatives,planning_teams ON projects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: projects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.external_project_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.planning_initiatives AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.planning_initiatives) THEN NEW.planning_initiatives END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.planning_teams AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.planning_teams) THEN NEW.planning_teams END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_relocated_task_processes_insert BEFORE INSERT ON relocated_task_processes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: relocated_task_processes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_relocated_task_processes_update BEFORE UPDATE OF process_id,task_id ON relocated_task_processes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: relocated_task_processes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_repository_plans_insert BEFORE INSERT ON repository_plans
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: repository_plans') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_repository_plans_update BEFORE UPDATE OF id ON repository_plans
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: repository_plans') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_session_activity_insert BEFORE INSERT ON session_activity
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: session_activity') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.attachment_token AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_session_activity_update BEFORE UPDATE OF attachment_token,agent_process_id,session_id ON session_activity
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: session_activity') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.attachment_token AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_session_events_insert BEFORE INSERT ON session_events
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: session_events') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
UNION ALL
SELECT CASE WHEN path='$.flow_id' AND json_extract(doc,'$.type') IN ('legacy_flow','legacy_flow_review') THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.payload AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.payload) THEN NEW.payload END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_session_events_update BEFORE UPDATE OF lf_process_id,wave_id,agent_process_id,task_id,session_id,payload ON session_events
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: session_events') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
UNION ALL
SELECT CASE WHEN path='$.flow_id' AND json_extract(doc,'$.type') IN ('legacy_flow','legacy_flow_review') THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.payload AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.payload) THEN NEW.payload END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_session_steers_insert BEFORE INSERT ON session_steers
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: session_steers') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.steer_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_session_steers_update BEFORE UPDATE OF steer_id,session_id ON session_steers
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: session_steers') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.steer_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_steer_adapters_insert BEFORE INSERT ON steer_adapters
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: steer_adapters') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_steer_adapters_update BEFORE UPDATE OF agent_process_id ON steer_adapters
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: steer_adapters') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_steer_batches_insert BEFORE INSERT ON steer_batches
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: steer_batches') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.attachment_token AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_steer_batches_update BEFORE UPDATE OF id,agent_process_id,attachment_token,session_id ON steer_batches
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: steer_batches') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.attachment_token AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_steer_deliveries_insert BEFORE INSERT ON steer_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: steer_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.steer_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.batch_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_steer_deliveries_update BEFORE UPDATE OF steer_id,batch_id,session_id ON steer_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: steer_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.steer_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.batch_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_steers_insert BEFORE INSERT ON steers
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: steers') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_steers_update BEFORE UPDATE OF id,wave_id,task_id,project_id,session_id ON steers
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: steers') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_admissions_insert BEFORE INSERT ON task_admissions
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_admissions') FROM (
 WITH incoming AS (
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'home_' AS domain,NEW.machine_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_admissions_update BEFORE UPDATE OF task_id,machine_id ON task_admissions
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_admissions') FROM (
 WITH incoming AS (
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'home_' AS domain,NEW.machine_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_changes_insert BEFORE INSERT ON task_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT CASE WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('comment_task') THEN 'task_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value' AND typeof(key)='integer' AND json_extract(doc,'$.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.machine_id' AND json_extract(doc,'$.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.repository_id' AND json_extract(doc,'$.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.value_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.value_json) THEN NEW.value_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.base_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.base_json) THEN NEW.base_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.conflict_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.conflict_json) THEN NEW.conflict_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_changes_update BEFORE UPDATE OF id,task_id,value_json,base_json,conflict_json ON task_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT CASE WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('comment_task') THEN 'task_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value' AND typeof(key)='integer' AND json_extract(doc,'$.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.machine_id' AND json_extract(doc,'$.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.repository_id' AND json_extract(doc,'$.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.value_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.value_json) THEN NEW.value_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.base_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.base_json) THEN NEW.base_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.conflict_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.conflict_json) THEN NEW.conflict_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_comment_deliveries_insert BEFORE INSERT ON task_comment_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_comment_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.comment_id AS original
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.comment_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.comment_json) THEN NEW.comment_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.conflicting_comment_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.conflicting_comment_json) THEN NEW.conflicting_comment_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_comment_deliveries_update BEFORE UPDATE OF comment_id,comment_json,conflicting_comment_json ON task_comment_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_comment_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.comment_id AS original
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.comment_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.comment_json) THEN NEW.comment_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.conflicting_comment_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.conflicting_comment_json) THEN NEW.conflicting_comment_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_comments_insert BEFORE INSERT ON task_comments
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_comments') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_comments_update BEFORE UPDATE OF id,task_id ON task_comments
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_comments') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_creation_intents_insert BEFORE INSERT ON task_creation_intents
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_creation_intents') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.repository_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' WHEN path='$.initiative' THEN 'uuid' WHEN path='$.link_id' THEN 'uuid' WHEN path='$.input.id' THEN 'uuid' WHEN path='$.input.teamId' THEN 'uuid' WHEN path='$.input.projectId' THEN 'uuid' WHEN path='$.input.assigneeId' THEN 'uuid' WHEN parent='$.input.teamIds' AND typeof(key)='integer' THEN 'uuid' WHEN (parent GLOB '$.baseline[[]*].value') AND typeof(key)='integer' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*].value' AND key='machine_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN parent GLOB '$.baseline[[]*].value' AND key='repository_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field')='task_admission_authority' THEN 'repo_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_project','task_project') THEN 'proj_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('comment_task') THEN 'task_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('task_identifier') THEN 'label' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.export_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.export_json) THEN NEW.export_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_creation_intents_update BEFORE UPDATE OF task_id,project_id,export_json,repository_id ON task_creation_intents
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_creation_intents') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.repository_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' WHEN path='$.initiative' THEN 'uuid' WHEN path='$.link_id' THEN 'uuid' WHEN path='$.input.id' THEN 'uuid' WHEN path='$.input.teamId' THEN 'uuid' WHEN path='$.input.projectId' THEN 'uuid' WHEN path='$.input.assigneeId' THEN 'uuid' WHEN parent='$.input.teamIds' AND typeof(key)='integer' THEN 'uuid' WHEN (parent GLOB '$.baseline[[]*].value') AND typeof(key)='integer' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*].value' AND key='machine_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN parent GLOB '$.baseline[[]*].value' AND key='repository_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field')='task_admission_authority' THEN 'repo_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_project','task_project') THEN 'proj_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('comment_task') THEN 'task_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('task_identifier') THEN 'label' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.export_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.export_json) THEN NEW.export_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_deletions_insert BEFORE INSERT ON task_deletions
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_deletions') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.issue_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.identifier AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_deletions_update BEFORE UPDATE OF wave_id,issue_id,identifier ON task_deletions
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_deletions') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.issue_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.identifier AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_events_insert BEFORE INSERT ON task_events
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_events') FROM (
 WITH incoming AS (
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT CASE WHEN path='$.pr_id' THEN 'pr_' WHEN path='$.steer_id' THEN 'uuid' WHEN path='$.invocation_id' THEN 'uuid' WHEN path='$.relation_id' THEN 'uuid' WHEN path='$.source_issue_id' THEN 'uuid' WHEN path='$.target_issue_id' THEN 'uuid' WHEN parent='$.lf_process_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.exec_ids' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.intent.issue_id' THEN 'task_or_uuid' WHEN path='$.intent.project_id' THEN 'project_or_uuid' WHEN path='$.intent.relation_id' THEN 'uuid' WHEN path='$.intent.team_id' THEN 'uuid' WHEN path='$.intent.state_id' THEN 'uuid' WHEN path='$.intent.identifier' THEN 'label' WHEN path='$.link.issue_id' THEN 'task_or_uuid' WHEN path='$.link.project_id' THEN 'project_or_uuid' WHEN path='$.link.relation_id' THEN 'uuid' WHEN path='$.link.team_id' THEN 'uuid' WHEN path='$.link.state_id' THEN 'uuid' WHEN path='$.link.identifier' THEN 'label' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.kind_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.kind_json) THEN NEW.kind_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_events_update BEFORE UPDATE OF task_id,kind_json ON task_events
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_events') FROM (
 WITH incoming AS (
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT CASE WHEN path='$.pr_id' THEN 'pr_' WHEN path='$.steer_id' THEN 'uuid' WHEN path='$.invocation_id' THEN 'uuid' WHEN path='$.relation_id' THEN 'uuid' WHEN path='$.source_issue_id' THEN 'uuid' WHEN path='$.target_issue_id' THEN 'uuid' WHEN parent='$.lf_process_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.exec_ids' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.intent.issue_id' THEN 'task_or_uuid' WHEN path='$.intent.project_id' THEN 'project_or_uuid' WHEN path='$.intent.relation_id' THEN 'uuid' WHEN path='$.intent.team_id' THEN 'uuid' WHEN path='$.intent.state_id' THEN 'uuid' WHEN path='$.intent.identifier' THEN 'label' WHEN path='$.link.issue_id' THEN 'task_or_uuid' WHEN path='$.link.project_id' THEN 'project_or_uuid' WHEN path='$.link.relation_id' THEN 'uuid' WHEN path='$.link.team_id' THEN 'uuid' WHEN path='$.link.state_id' THEN 'uuid' WHEN path='$.link.identifier' THEN 'label' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.kind_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.kind_json) THEN NEW.kind_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_issue_identities_insert BEFORE INSERT ON task_issue_identities
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_issue_identities') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.issue_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.identifier AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_issue_identities_update BEFORE UPDATE OF wave_id,issue_id,identifier ON task_issue_identities
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_issue_identities') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.issue_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.identifier AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_linear_ingested_comments_insert BEFORE INSERT ON task_linear_ingested_comments
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_linear_ingested_comments') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.comment_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_linear_ingested_comments_update BEFORE UPDATE OF comment_id,task_id ON task_linear_ingested_comments
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_linear_ingested_comments') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.comment_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_linear_observations_insert BEFORE INSERT ON task_linear_observations
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_linear_observations') FROM (
 WITH incoming AS (
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_linear_observations_update BEFORE UPDATE OF task_id ON task_linear_observations
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_linear_observations') FROM (
 WITH incoming AS (
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_pr_repair_incidents_insert BEFORE INSERT ON task_pr_repair_incidents
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_pr_repair_incidents') FROM (
 WITH incoming AS (
SELECT 'pr_' AS domain,NEW.task_pr_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_pr_repair_incidents_update BEFORE UPDATE OF task_pr_id ON task_pr_repair_incidents
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_pr_repair_incidents') FROM (
 WITH incoming AS (
SELECT 'pr_' AS domain,NEW.task_pr_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_prs_insert BEFORE INSERT ON task_prs
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_prs') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.linear_attachment_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.linear_comment_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.parent_pr_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_prs_update BEFORE UPDATE OF linear_attachment_id,linear_comment_id,task_id,id,parent_pr_id ON task_prs
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_prs') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.linear_attachment_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.linear_comment_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.parent_pr_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_state_deliveries_insert BEFORE INSERT ON task_state_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_state_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_state_deliveries_update BEFORE UPDATE OF id,task_id ON task_state_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_state_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_workflow_moves_insert BEFORE INSERT ON task_workflow_moves
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_workflow_moves') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.lf_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_workflow_moves_update BEFORE UPDATE OF lf_process_id,task_id ON task_workflow_moves
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_workflow_moves') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.lf_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_workflows_insert BEFORE INSERT ON task_workflows
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_workflows') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.lf_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_task_workflows_update BEFORE UPDATE OF lf_process_id,task_id ON task_workflows
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_workflows') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.lf_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
SELECT 'home_' AS domain,NEW.checkout_machine_id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.parent_pr_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.primary_session_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.issue_identifier AS original
UNION ALL
SELECT CASE WHEN path='$.machine_id' THEN 'home_' WHEN path='$.repository_id' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.admission_authority AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.admission_authority) THEN NEW.admission_authority END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_tasks_update BEFORE UPDATE OF external_issue_id,planning_assignee,planning_team_id,id,project_id,checkout_machine_id,parent_pr_id,primary_session_id,issue_identifier,admission_authority,repository_id ON tasks
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
SELECT 'home_' AS domain,NEW.checkout_machine_id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.parent_pr_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.primary_session_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.issue_identifier AS original
UNION ALL
SELECT CASE WHEN path='$.machine_id' THEN 'home_' WHEN path='$.repository_id' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.admission_authority AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.admission_authority) THEN NEW.admission_authority END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_tool_responses_insert BEFORE INSERT ON tool_responses
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tool_responses') FROM (
 WITH incoming AS (
SELECT 'response_' AS domain,NEW.id AS original
UNION ALL
SELECT CASE NEW.work_kind WHEN 'wave' THEN 'uuid' WHEN 'project' THEN 'proj_' WHEN 'task' THEN 'task_' END AS domain,NEW.work_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_tool_responses_update BEFORE UPDATE OF id,work_id ON tool_responses
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tool_responses') FROM (
 WITH incoming AS (
SELECT 'response_' AS domain,NEW.id AS original
UNION ALL
SELECT CASE NEW.work_kind WHEN 'wave' THEN 'uuid' WHEN 'project' THEN 'proj_' WHEN 'task' THEN 'task_' END AS domain,NEW.work_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_wave_workflows_insert BEFORE INSERT ON wave_workflows
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: wave_workflows') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_wave_workflows_update BEFORE UPDATE OF wave_id ON wave_workflows
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: wave_workflows') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_waves_insert BEFORE INSERT ON waves
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: waves') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.parent_wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.superseded_by_wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_activation_lf_process_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.current_project_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_waves_update BEFORE UPDATE OF id,parent_wave_id,superseded_by_wave_id,project_activation_lf_process_id,current_project_id ON waves
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: waves') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.parent_wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.superseded_by_wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_activation_lf_process_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.current_project_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_work_placements_insert BEFORE INSERT ON work_placements
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: work_placements') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'home_' AS domain,NEW.machine_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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

CREATE TRIGGER canonical_work_placements_update BEFORE UPDATE OF wave_id,task_id,project_id,machine_id ON work_placements
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: work_placements') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'home_' AS domain,NEW.machine_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
