-- depends_on: process_names
PRAGMA legacy_alter_table = ON;

-- Local planning keeps durable Work identity independent of provider mapping.
-- Foreign-key actions are disabled by the migration runner during table rebuilds.

CREATE TABLE projects_migration (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    external_project_id TEXT UNIQUE,
    created_at INTEGER NOT NULL,
    project_slug TEXT,
    project_name TEXT,
    project_prompt_context TEXT,
    pm_snapshot_synced_at INTEGER,
    abandon_requested_at INTEGER,
    abandon_reason TEXT,
    updated_at INTEGER,
    work_state TEXT NOT NULL DEFAULT 'ready'
        CHECK (work_state IN ('ready', 'done', 'abandoned')),
    work_terminal_at INTEGER,
    iteration INTEGER NOT NULL DEFAULT 0 CHECK (iteration >= 0),
    status TEXT NOT NULL DEFAULT 'started'
        CHECK (status IN ('backlog','planned','started','paused','completed','canceled')),
    workflow TEXT NOT NULL DEFAULT '',
    legacy_current INTEGER CHECK (legacy_current IN (-1, 0, 1))
);

INSERT INTO projects_migration (
    id, wave_id, external_project_id, created_at,
    project_slug, project_name, project_prompt_context, pm_snapshot_synced_at,
    abandon_requested_at, abandon_reason, updated_at, work_state,
    work_terminal_at, iteration, status, workflow,
    legacy_current
)
SELECT
    id, wave_id, external_project_id, created_at,
    project_slug, project_name, project_prompt_context, pm_snapshot_synced_at,
    abandon_requested_at, abandon_reason, updated_at, work_state,
    work_terminal_at, iteration, status, workflow,
    legacy_current
FROM projects;

DROP TABLE projects;

ALTER TABLE projects_migration RENAME TO projects;

CREATE INDEX idx_projects_wave ON projects(wave_id, created_at);

CREATE INDEX idx_projects_wave_updated ON projects(wave_id, updated_at DESC);

CREATE TRIGGER validate_project_conversation_parents BEFORE UPDATE OF wave_id ON projects
WHEN EXISTS(SELECT 1 FROM agent_sessions s JOIN tasks t ON t.id=s.task_id
    WHERE t.project_id=NEW.id AND s.wave_id IS NOT NEW.wave_id)
BEGIN SELECT RAISE(ABORT,'Project would change AgentSession ancestry'); END;

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

CREATE TRIGGER selected_project_owner_update BEFORE UPDATE OF wave_id ON projects
WHEN EXISTS(SELECT 1 FROM waves WHERE current_project_id=NEW.id AND id!=NEW.wave_id)
BEGIN SELECT RAISE(ABORT, 'selected Project cannot change Wave ownership'); END;

CREATE TABLE tasks_migration (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    external_issue_id TEXT UNIQUE,
    issue_identifier TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    issue_title TEXT,
    issue_description TEXT,
    pm_snapshot_synced_at INTEGER,
    pm_writeback_json TEXT,
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
    planning_revision INTEGER NOT NULL DEFAULT 0 CHECK (planning_revision >= 0)
);

INSERT INTO tasks_migration (
    id, project_id, external_issue_id, issue_identifier,
    created_at, issue_title, issue_description, pm_snapshot_synced_at,
    pm_writeback_json, worktree, workspace_slug, abandon_requested_at,
    abandon_reason, updated_at, agent, started_at,
    automation_enabled, abandoned_at, primary_session_id
)
SELECT
    id, project_id, external_issue_id, issue_identifier,
    created_at, issue_title, issue_description, pm_snapshot_synced_at,
    pm_writeback_json, worktree, workspace_slug, abandon_requested_at,
    abandon_reason, updated_at, agent, started_at,
    automation_enabled, abandoned_at, primary_session_id
FROM tasks;

DROP TABLE tasks;

ALTER TABLE tasks_migration RENAME TO tasks;

CREATE INDEX idx_tasks_project ON tasks(project_id, created_at);

CREATE UNIQUE INDEX idx_tasks_issue_identifier ON tasks(issue_identifier);

CREATE UNIQUE INDEX idx_tasks_worktree ON tasks(worktree);

CREATE INDEX idx_tasks_updated ON tasks(updated_at DESC);

CREATE TRIGGER validate_task_started_insert BEFORE INSERT ON tasks
WHEN NEW.started_at IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'Started requires a Task Run'); END;

CREATE TRIGGER validate_task_conversation_parents BEFORE UPDATE OF project_id ON tasks
WHEN EXISTS(SELECT 1 FROM agent_sessions s JOIN projects p ON p.id=NEW.project_id
    WHERE s.task_id=NEW.id AND s.wave_id IS NOT p.wave_id)
BEGIN SELECT RAISE(ABORT,'Task would change AgentSession ancestry'); END;

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
        EXISTS(SELECT 1 FROM flow_processes f JOIN processes driver ON driver.lfid=f.process_lfid WHERE NEW.worktree!=''
            AND (driver.cwd=rtrim(NEW.worktree,'/') OR instr(driver.cwd,rtrim(NEW.worktree,'/')||'/')=1)) OR
        EXISTS(SELECT 1 FROM task_events e WHERE e.task_id=NEW.id
            AND json_extract(e.kind_json,'$.kind')='started')
    ) THEN RAISE(ABORT,'Started requires recorded Task work') END;
END;

CREATE TABLE task_creation_intents (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE RESTRICT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    title TEXT NOT NULL,
    description TEXT NOT NULL
);

PRAGMA legacy_alter_table = OFF;
