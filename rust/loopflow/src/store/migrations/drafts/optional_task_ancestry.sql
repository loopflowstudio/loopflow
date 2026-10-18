-- Tasks retain repository identity independently of planning membership.

PRAGMA legacy_alter_table = ON;

CREATE TABLE tasks_migration (
    id TEXT PRIMARY KEY,
    repository_id TEXT NOT NULL REFERENCES repository_plans(id),
    project_id TEXT REFERENCES projects(id) ON DELETE RESTRICT,
    external_issue_id TEXT UNIQUE,
    issue_identifier TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    issue_title TEXT,
    issue_description TEXT,
    pm_snapshot_synced_at INTEGER,
    worktree TEXT,
    workspace_slug TEXT,
    abandon_requested_at INTEGER,
    abandon_reason TEXT,
    updated_at INTEGER,
    agent TEXT,
    started_at INTEGER,
    automation_enabled INTEGER CHECK (automation_enabled IN (0,1)),
    abandoned_at INTEGER,
    primary_session_id TEXT REFERENCES agent_sessions(id) ON DELETE SET NULL,
    planning_revision INTEGER NOT NULL DEFAULT 0 CHECK (planning_revision >= 0),
    planning_rank INTEGER NOT NULL DEFAULT 0 CHECK (planning_rank >= 0),
    planning_assignee TEXT,
    planning_state TEXT,
    planning_completed_at TEXT,
    planning_provider_revision TEXT,
    planning_url TEXT,
    planning_branch_name TEXT,
    planning_team_id TEXT,
    planning_completed INTEGER NOT NULL DEFAULT 0 CHECK (planning_completed IN (0,1)),
    planning_deleted_at INTEGER
, branch TEXT NOT NULL DEFAULT '', base_commit TEXT NOT NULL DEFAULT '', parent_pr_id TEXT REFERENCES task_prs(id), completion_request INTEGER, completion_error TEXT, planning_due_date TEXT, checkout_machine_id TEXT REFERENCES machines(id), admission_authority TEXT CHECK(admission_authority IS NULL OR json_valid(admission_authority)), paused INTEGER NOT NULL DEFAULT 0 CHECK (paused IN (0,1)));

INSERT INTO tasks_migration (id, project_id, external_issue_id, issue_identifier, created_at, issue_title, issue_description, pm_snapshot_synced_at, worktree, workspace_slug, abandon_requested_at, abandon_reason, updated_at, agent, started_at, automation_enabled, abandoned_at, primary_session_id, planning_revision, planning_rank, planning_assignee, planning_state, planning_completed_at, planning_provider_revision, planning_url, planning_branch_name, planning_team_id, planning_completed, planning_deleted_at, branch, base_commit, parent_pr_id, completion_request, completion_error, planning_due_date, checkout_machine_id, admission_authority, paused, repository_id)
SELECT t.id, t.project_id, t.external_issue_id, t.issue_identifier, t.created_at, t.issue_title, t.issue_description, t.pm_snapshot_synced_at, t.worktree, t.workspace_slug, t.abandon_requested_at, t.abandon_reason, t.updated_at, t.agent, t.started_at, t.automation_enabled, t.abandoned_at, t.primary_session_id, t.planning_revision, t.planning_rank, t.planning_assignee, t.planning_state, t.planning_completed_at, t.planning_provider_revision, t.planning_url, t.planning_branch_name, t.planning_team_id, t.planning_completed, t.planning_deleted_at, t.branch, t.base_commit, t.parent_pr_id, t.completion_request, t.completion_error, t.planning_due_date, t.checkout_machine_id, t.admission_authority, t.paused,
    (SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id
     JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=t.project_id)
FROM tasks t;

DROP TABLE tasks;

ALTER TABLE tasks_migration RENAME TO tasks;

CREATE INDEX idx_tasks_project ON tasks(project_id, created_at);

CREATE INDEX idx_tasks_issue_identifier ON tasks(issue_identifier);

CREATE UNIQUE INDEX idx_tasks_worktree ON tasks(worktree);

CREATE INDEX idx_tasks_updated ON tasks(updated_at DESC);

CREATE TRIGGER validate_task_started_insert BEFORE INSERT ON tasks
WHEN NEW.started_at IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'Started requires a Task Run'); END;

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

CREATE TRIGGER task_placement_to_pr AFTER UPDATE OF branch,base_commit,parent_pr_id ON tasks
BEGIN
 UPDATE task_prs SET branch=NEW.branch, base_commit=NEW.base_commit,
 parent_pr_id=NEW.parent_pr_id WHERE task_id=NEW.id AND historical=0
 AND merge_commit IS NULL AND abandoned_at IS NULL;
END;

CREATE INDEX tasks_branch ON tasks(branch);

CREATE TABLE task_creation_intents_migration (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE RESTRICT,
    repository_id TEXT NOT NULL REFERENCES repository_plans(id),
    project_id TEXT REFERENCES projects(id) ON DELETE RESTRICT,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    export_json TEXT CHECK(export_json IS NULL OR json_valid(export_json)),
    export_attempted INTEGER NOT NULL DEFAULT 0,
    export_error TEXT
, due_date TEXT);

INSERT INTO task_creation_intents_migration (task_id, project_id, title, description, export_json, export_attempted, export_error, due_date, repository_id)
SELECT t.task_id, t.project_id, t.title, t.description, t.export_json, t.export_attempted, t.export_error, t.due_date,
    (SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id
     JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=t.project_id)
FROM task_creation_intents t;

DROP TABLE task_creation_intents;

ALTER TABLE task_creation_intents_migration RENAME TO task_creation_intents;

CREATE TRIGGER store_revision_task_creation_intents_insert AFTER INSERT ON task_creation_intents
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_task_creation_intents_update AFTER UPDATE ON task_creation_intents
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_task_creation_intents_delete AFTER DELETE ON task_creation_intents
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE INDEX idx_tasks_repository ON tasks(repository_id, project_id, planning_rank);

DROP TRIGGER validate_project_conversation_parents;

CREATE TRIGGER validate_task_repository_insert BEFORE INSERT ON tasks
WHEN NEW.project_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM projects p JOIN waves w ON w.id=p.wave_id
    JOIN repository_plans r ON r.repo=w.repo WHERE p.id=NEW.project_id AND r.id=NEW.repository_id
)
BEGIN SELECT RAISE(ABORT, 'Task Project must belong to its repository'); END;

CREATE TRIGGER validate_task_repository_update BEFORE UPDATE OF project_id,repository_id ON tasks
WHEN NEW.project_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM projects p JOIN waves w ON w.id=p.wave_id
    JOIN repository_plans r ON r.repo=w.repo WHERE p.id=NEW.project_id AND r.id=NEW.repository_id
)
BEGIN SELECT RAISE(ABORT, 'Task Project must belong to its repository'); END;

CREATE TRIGGER task_repository_is_fixed BEFORE UPDATE OF repository_id ON tasks
WHEN NEW.repository_id IS NOT OLD.repository_id
BEGIN SELECT RAISE(ABORT, 'Task repository identity cannot change'); END;

CREATE TRIGGER validate_project_task_repositories BEFORE UPDATE OF wave_id ON projects
WHEN EXISTS (SELECT 1 FROM tasks t JOIN repository_plans r ON r.id=t.repository_id
    WHERE t.project_id=NEW.id AND NOT EXISTS(SELECT 1 FROM waves w WHERE w.id=NEW.wave_id AND w.repo=r.repo))
BEGIN SELECT RAISE(ABORT, 'Project move would change Task repository'); END;

-- Current Project selection alone supplies the initial Wave default.
ALTER TABLE waves ADD COLUMN workflow TEXT;
UPDATE waves SET workflow=(SELECT NULLIF(trim(p.workflow),'') FROM projects p WHERE p.id=waves.current_project_id);
ALTER TABLE projects DROP COLUMN workflow;
ALTER TABLE pm_projects DROP COLUMN workflow;
-- Retain immutable old journal values as evidence; no pending Project default can be delivered.
UPDATE project_changes SET acknowledged=1 WHERE field='workflow';
PRAGMA legacy_alter_table = OFF;

-- A Session remains Task-bound. Current membership may change; captures retain history.
DROP TRIGGER validate_conversation_ancestry_insert;
DROP TRIGGER validate_conversation_ancestry_update;
CREATE TRIGGER validate_conversation_ancestry_insert BEFORE INSERT ON agent_sessions BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM tasks t LEFT JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id IS NEW.wave_id
    ) THEN RAISE(ABORT,'Session Task and Wave disagree') END;
END;
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
CREATE TRIGGER task_session_membership AFTER UPDATE OF project_id ON tasks BEGIN
    UPDATE agent_sessions SET wave_id=(SELECT wave_id FROM projects WHERE id=NEW.project_id) WHERE task_id=NEW.id;
END;

CREATE TRIGGER project_task_session_membership AFTER UPDATE OF wave_id ON projects BEGIN
    UPDATE agent_sessions SET wave_id=NEW.wave_id WHERE task_id IN (SELECT id FROM tasks WHERE project_id=NEW.id);
END;
