-- Change revisions for displayed data. Every committed write to a table a
-- workspace surface reads bumps its domain here, inside the writer's own
-- transaction, so any reader can ask "what changed" without scanning history.
-- A domain follows what a write can change on screen, not which table it
-- lands in: provider transcript lines and usage events bump nothing.
CREATE TABLE store_revisions (
    domain TEXT PRIMARY KEY NOT NULL CHECK (domain IN ('planning', 'sessions', 'flows', 'execs')),
    revision INTEGER NOT NULL
) WITHOUT ROWID;
INSERT INTO store_revisions(domain, revision)
VALUES ('planning', 0), ('sessions', 0), ('flows', 0), ('execs', 0);

-- The completion gate asks only about unfinished execution. One statement
-- pairs the few unfinished Execs with their Sessions and Flows, and these
-- answer it from the index without reading an event row.
CREATE INDEX execs_unfinished ON execs(started_at, id) WHERE completed_at IS NULL;
CREATE INDEX session_events_exec ON session_events(exec_id, session_id) WHERE exec_id IS NOT NULL;
CREATE INDEX flow_events_exec ON flow_events(exec_id, flow_id) WHERE exec_id IS NOT NULL;


-- planning
CREATE TRIGGER store_revision_waves_insert AFTER INSERT ON waves
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_waves_update AFTER UPDATE ON waves
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_waves_delete AFTER DELETE ON waves
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_projects_insert AFTER INSERT ON projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_projects_update AFTER UPDATE ON projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_projects_delete AFTER DELETE ON projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tasks_insert AFTER INSERT ON tasks
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tasks_update AFTER UPDATE ON tasks
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tasks_delete AFTER DELETE ON tasks
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_prs_insert AFTER INSERT ON task_prs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_prs_update AFTER UPDATE ON task_prs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_prs_delete AFTER DELETE ON task_prs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_items_insert AFTER INSERT ON pm_items
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_items_update AFTER UPDATE ON pm_items
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_items_delete AFTER DELETE ON pm_items
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_projects_insert AFTER INSERT ON pm_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_projects_update AFTER UPDATE ON pm_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_projects_delete AFTER DELETE ON pm_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_wave_projects_insert AFTER INSERT ON pm_wave_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_wave_projects_update AFTER UPDATE ON pm_wave_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_wave_projects_delete AFTER DELETE ON pm_wave_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_wave_sync_insert AFTER INSERT ON pm_wave_sync
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_wave_sync_update AFTER UPDATE ON pm_wave_sync
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_wave_sync_delete AFTER DELETE ON pm_wave_sync
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_issue_changes_insert AFTER INSERT ON pm_issue_changes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_issue_changes_update AFTER UPDATE ON pm_issue_changes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_issue_changes_delete AFTER DELETE ON pm_issue_changes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_metric_instruments_insert AFTER INSERT ON metric_instruments
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_metric_instruments_update AFTER UPDATE ON metric_instruments
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_metric_instruments_delete AFTER DELETE ON metric_instruments
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_metric_observations_insert AFTER INSERT ON metric_observations
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_metric_observations_update AFTER UPDATE ON metric_observations
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_metric_observations_delete AFTER DELETE ON metric_observations
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_project_events_insert AFTER INSERT ON project_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_project_events_update AFTER UPDATE ON project_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_project_events_delete AFTER DELETE ON project_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_events_insert AFTER INSERT ON task_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_events_update AFTER UPDATE ON task_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_events_delete AFTER DELETE ON task_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_deletions_insert AFTER INSERT ON task_deletions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_deletions_update AFTER UPDATE ON task_deletions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_deletions_delete AFTER DELETE ON task_deletions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_issue_identities_insert AFTER INSERT ON task_issue_identities
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_issue_identities_update AFTER UPDATE ON task_issue_identities
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_issue_identities_delete AFTER DELETE ON task_issue_identities
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_linear_observations_insert AFTER INSERT ON task_linear_observations
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_linear_observations_update AFTER UPDATE ON task_linear_observations
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_linear_observations_delete AFTER DELETE ON task_linear_observations
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_pr_repair_incidents_insert AFTER INSERT ON task_pr_repair_incidents
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_pr_repair_incidents_update AFTER UPDATE ON task_pr_repair_incidents
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_pr_repair_incidents_delete AFTER DELETE ON task_pr_repair_incidents
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_work_placements_insert AFTER INSERT ON work_placements
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_work_placements_update AFTER UPDATE ON work_placements
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_work_placements_delete AFTER DELETE ON work_placements
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pr_landings_insert AFTER INSERT ON pr_landings
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pr_landings_update AFTER UPDATE ON pr_landings
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pr_landings_delete AFTER DELETE ON pr_landings
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_ci_incidents_insert AFTER INSERT ON ci_incidents
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_ci_incidents_update AFTER UPDATE ON ci_incidents
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_ci_incidents_delete AFTER DELETE ON ci_incidents
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;

-- sessions
CREATE TRIGGER store_revision_agent_sessions_insert AFTER INSERT ON agent_sessions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_agent_sessions_update AFTER UPDATE ON agent_sessions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_agent_sessions_delete AFTER DELETE ON agent_sessions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;

-- flows
CREATE TRIGGER store_revision_flow_sessions_insert AFTER INSERT ON flow_sessions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'flows';
END;
CREATE TRIGGER store_revision_flow_sessions_update AFTER UPDATE ON flow_sessions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'flows';
END;
CREATE TRIGGER store_revision_flow_sessions_delete AFTER DELETE ON flow_sessions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'flows';
END;
CREATE TRIGGER store_revision_flow_events_insert AFTER INSERT ON flow_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'flows';
END;
CREATE TRIGGER store_revision_flow_events_update AFTER UPDATE ON flow_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'flows';
END;
CREATE TRIGGER store_revision_flow_events_delete AFTER DELETE ON flow_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'flows';
END;

-- execs
CREATE TRIGGER store_revision_execs_insert AFTER INSERT ON execs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'execs';
END;
CREATE TRIGGER store_revision_execs_update AFTER UPDATE ON execs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'execs';
END;
CREATE TRIGGER store_revision_execs_delete AFTER DELETE ON execs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'execs';
END;

-- sessions: every Session event except usage and transcript lines. A transcript
-- line is an `events.jsonl` observation of a type no summary reader selects;
-- provider attempts and identities share that receipt key and are displayed.
-- Token totals therefore follow the next displayed change, not each usage row.
CREATE TRIGGER store_revision_session_events_insert AFTER INSERT ON session_events
WHEN NOT (NEW.kind = 'usage' OR (NEW.kind = 'observed' AND instr(NEW.receipt_key, ':events.jsonl:') > 0
    AND CASE WHEN json_valid(NEW.payload) THEN COALESCE(json_extract(NEW.payload, '$.evidence.schema_version'), 0) = 1
        AND COALESCE(json_extract(NEW.payload, '$.evidence.type'), '') IN
            ('activity', 'handoff', 'user_input', 'conversation', 'text', 'tool_use', 'result', 'provider_output', 'usage')
        ELSE 0 END))
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_session_events_update AFTER UPDATE ON session_events
WHEN NOT (NEW.kind = 'usage' OR (NEW.kind = 'observed' AND instr(NEW.receipt_key, ':events.jsonl:') > 0
    AND CASE WHEN json_valid(NEW.payload) THEN COALESCE(json_extract(NEW.payload, '$.evidence.schema_version'), 0) = 1
        AND COALESCE(json_extract(NEW.payload, '$.evidence.type'), '') IN
            ('activity', 'handoff', 'user_input', 'conversation', 'text', 'tool_use', 'result', 'provider_output', 'usage')
        ELSE 0 END))
    OR NOT (OLD.kind = 'usage' OR (OLD.kind = 'observed' AND instr(OLD.receipt_key, ':events.jsonl:') > 0
    AND CASE WHEN json_valid(OLD.payload) THEN COALESCE(json_extract(OLD.payload, '$.evidence.schema_version'), 0) = 1
        AND COALESCE(json_extract(OLD.payload, '$.evidence.type'), '') IN
            ('activity', 'handoff', 'user_input', 'conversation', 'text', 'tool_use', 'result', 'provider_output', 'usage')
        ELSE 0 END))
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_session_events_delete AFTER DELETE ON session_events
WHEN NOT (OLD.kind = 'usage' OR (OLD.kind = 'observed' AND instr(OLD.receipt_key, ':events.jsonl:') > 0
    AND CASE WHEN json_valid(OLD.payload) THEN COALESCE(json_extract(OLD.payload, '$.evidence.schema_version'), 0) = 1
        AND COALESCE(json_extract(OLD.payload, '$.evidence.type'), '') IN
            ('activity', 'handoff', 'user_input', 'conversation', 'text', 'tool_use', 'result', 'provider_output', 'usage')
        ELSE 0 END))
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
