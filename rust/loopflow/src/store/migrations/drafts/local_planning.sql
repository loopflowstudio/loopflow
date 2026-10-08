-- depends_on: process_names
PRAGMA legacy_alter_table = ON;

CREATE TABLE personal_plans (
    id TEXT PRIMARY KEY,
    repo TEXT NOT NULL UNIQUE
);
ALTER TABLE waves ADD COLUMN personal_plan_id TEXT REFERENCES personal_plans(id) ON DELETE RESTRICT;
ALTER TABLE project_transitions ADD COLUMN local_plan_json TEXT;
CREATE TABLE personal_wave_definitions (
    wave_id TEXT PRIMARY KEY REFERENCES waves(id) ON DELETE RESTRICT,
    goal TEXT NOT NULL,
    memory TEXT NOT NULL
);
CREATE TABLE wave_workflows (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    PRIMARY KEY(wave_id,name)
);
DROP INDEX idx_waves_active_locator;
CREATE UNIQUE INDEX idx_waves_active_locator
    ON waves(repo,ifnull(personal_plan_id,''),ifnull(parent_wave_id,''),name)
    WHERE retired_at IS NULL;
DROP VIEW wave_addresses;
CREATE VIEW wave_addresses AS
WITH RECURSIVE addresses(id,slug) AS (
    SELECT id,CASE
        WHEN personal_plan_id IS NOT NULL THEN 'personal:' || name
        WHEN name LIKE 'personal:%' OR name LIKE 'shared:%' THEN 'shared:' || name
        ELSE name END
    FROM waves WHERE parent_wave_id IS NULL
    UNION ALL
    SELECT w.id,a.slug || '/' || w.name FROM waves w
    JOIN addresses a ON w.parent_wave_id=a.id
)
SELECT w.*,a.slug FROM waves w JOIN addresses a ON a.id=w.id;

-- Local planning keeps durable Work identity independent of provider mapping.
-- Foreign-key actions are disabled by the migration runner during table rebuilds.

CREATE TABLE projects_migration (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    external_project_id TEXT UNIQUE,
    created_at INTEGER NOT NULL,
    project_slug TEXT,
    project_name TEXT,
    project_summary TEXT NOT NULL DEFAULT '',
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
    planning_revision INTEGER NOT NULL DEFAULT 0 CHECK (planning_revision >= 0),
    planning_rank INTEGER NOT NULL DEFAULT 0 CHECK (planning_rank >= 0),
    planning_assignee TEXT,
    planning_deleted_at INTEGER
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

CREATE INDEX idx_tasks_issue_identifier ON tasks(issue_identifier);

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

CREATE TABLE task_comments (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
    body TEXT NOT NULL,
    author TEXT NOT NULL CHECK(json_valid(author)),
    created_at TEXT,
    provider_revision TEXT
);
CREATE TABLE task_comment_deliveries (
    comment_id TEXT PRIMARY KEY REFERENCES task_comments(id) ON DELETE RESTRICT,
    body TEXT NOT NULL,
    acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged IN (0,1)),
    error TEXT,
    conflicting_body TEXT
);
CREATE INDEX idx_task_comments_task ON task_comments(task_id,created_at,id);
CREATE TRIGGER store_revision_task_comments_insert AFTER INSERT ON task_comments
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

-- Import only accepted, owned issues. UUID v4 identity is independent of provider
-- IDs; existing mappings and orphan deletion-recovery evidence stay untouched.
INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,issue_title,
    issue_description,pm_snapshot_synced_at,created_at,updated_at,planning_rank,workspace_slug,pm_writeback_json)
SELECT 'task_' || lower(hex(randomblob(6))) || '4' || substr(lower(hex(randomblob(2))),2) ||
    substr('89ab',(random() & 3)+1,1) || substr(lower(hex(randomblob(2))),2) || lower(hex(randomblob(6))),
    p.id,i.id,json_extract(i.body,'$.identifier'),json_extract(i.body,'$.name'),
    json_extract(i.body,'$.description'),i.observed_at,i.observed_at,i.observed_at,
    json_extract(i.body,'$.rank'),'','{"state":"current"}'
FROM pm_items i
JOIN projects p ON p.external_project_id=i.project_id
JOIN waves w ON w.id=p.wave_id AND w.repo=i.repo
JOIN pm_projects observed ON observed.id=i.project_id AND observed.repo=i.repo AND observed.provider=i.provider
JOIN pm_wave_projects membership ON membership.wave_id=w.id AND membership.project_id=i.project_id
JOIN pm_wave_sync sync ON sync.wave_id=w.id AND sync.provider=i.provider
WHERE i.provider='linear' AND i.needs_refresh=0
    AND observed.archived=0 AND observed.membership_unresolved=0
    AND json_array_length(observed.body,'$.initiative_ids')=1
    AND json_extract(observed.body,'$.initiative_ids[0]')=sync.initiative
    AND NOT EXISTS(SELECT 1 FROM tasks t WHERE t.external_issue_id=i.id)
    AND NOT EXISTS(SELECT 1 FROM task_deletions d WHERE d.wave_id=w.id AND d.issue_id=i.id)
    AND NOT EXISTS(SELECT 1 FROM pm_issue_changes c WHERE c.issue_id=i.id AND c.removed=1);

UPDATE projects SET project_summary=COALESCE((SELECT json_extract(observed.body,'$.summary')
    FROM pm_projects observed JOIN waves w ON w.id=projects.wave_id
    WHERE observed.id=projects.external_project_id AND observed.repo=w.repo
        AND observed.provider='linear'),'');

PRAGMA legacy_alter_table = OFF;

CREATE TRIGGER store_revision_personal_wave_definitions_insert AFTER INSERT ON personal_wave_definitions
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_personal_wave_definitions_update AFTER UPDATE ON personal_wave_definitions
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_personal_wave_definitions_delete AFTER DELETE ON personal_wave_definitions
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_wave_workflows_insert AFTER INSERT ON wave_workflows
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_wave_workflows_update AFTER UPDATE ON wave_workflows
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_wave_workflows_delete AFTER DELETE ON wave_workflows
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_task_comments_update AFTER UPDATE ON task_comments
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_task_comments_delete AFTER DELETE ON task_comments
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_task_comment_deliveries_insert AFTER INSERT ON task_comment_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_comment_deliveries_update AFTER UPDATE ON task_comment_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_task_comment_deliveries_delete AFTER DELETE ON task_comment_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
