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
    planning_rank INTEGER NOT NULL DEFAULT 0 CHECK(planning_rank >= 0),
    planning_provider_revision TEXT,
    planning_initiatives TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(planning_initiatives)),
    planning_teams TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(planning_teams)),
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

-- Delivery metadata belongs to the decision that produced it. Superseding a
-- decision preserves its attempted effect, without granting it write authority.
CREATE TABLE task_state_deliveries (
    seq INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
    move_seq INTEGER REFERENCES task_workflow_moves(seq),
    target TEXT NOT NULL CHECK(target IN ('completed','unstarted','canceled')),
    base_revision TEXT,
    base_state TEXT,
    attempted INTEGER NOT NULL DEFAULT 0 CHECK(attempted IN (0,1)),
    settled INTEGER NOT NULL DEFAULT 0 CHECK(settled IN (0,1)),
    error TEXT,
    conflict_json TEXT CHECK(conflict_json IS NULL OR json_valid(conflict_json))
);
CREATE INDEX task_state_deliveries_task ON task_state_deliveries(task_id,seq);

INSERT INTO task_state_deliveries(id,task_id,move_seq,target,attempted,error)
SELECT lower(hex(randomblob(16))),t.id,max(m.seq),
    CASE json_extract(t.pm_writeback_json,'$.operation')
        WHEN 'complete_task' THEN 'completed' WHEN 'reopen_task' THEN 'unstarted' END,
    1,json_extract(t.pm_writeback_json,'$.error')
FROM tasks t LEFT JOIN task_workflow_moves m ON m.task_id=t.id
WHERE json_extract(t.pm_writeback_json,'$.state')='pending'
GROUP BY t.id;

CREATE TABLE tasks_migration (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
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
);

INSERT INTO tasks_migration (
    id, project_id, external_issue_id, issue_identifier,
    created_at, issue_title, issue_description, pm_snapshot_synced_at,
    worktree, workspace_slug, abandon_requested_at,
    abandon_reason, updated_at, agent, started_at,
    automation_enabled, abandoned_at, primary_session_id
)
SELECT
    id, project_id, external_issue_id, issue_identifier,
    created_at, issue_title, issue_description, pm_snapshot_synced_at,
    worktree, workspace_slug, abandon_requested_at,
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
    comment_json TEXT NOT NULL CHECK(json_valid(comment_json)),
    acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged IN (0,1)),
    error TEXT,
    conflicting_comment_json TEXT CHECK(conflicting_comment_json IS NULL OR json_valid(conflicting_comment_json)),
    resolution TEXT CHECK(resolution IN ('local','linear')),
    replacement_comment_id TEXT REFERENCES task_comments(id) ON DELETE RESTRICT
);
CREATE INDEX idx_task_comments_task ON task_comments(task_id,created_at,id);
CREATE TRIGGER store_revision_task_comments_insert AFTER INSERT ON task_comments
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

-- Import only accepted, owned issues. UUID v4 identity is independent of provider
-- IDs; existing mappings and orphan deletion-recovery evidence stay untouched.
INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,issue_title,
    issue_description,pm_snapshot_synced_at,created_at,updated_at,planning_rank,workspace_slug)
SELECT 'task_' || lower(hex(randomblob(6))) || '4' || substr(lower(hex(randomblob(2))),2) ||
    substr('89ab',(random() & 3)+1,1) || substr(lower(hex(randomblob(2))),2) || lower(hex(randomblob(6))),
    p.id,i.id,json_extract(i.body,'$.identifier'),json_extract(i.body,'$.name'),
    json_extract(i.body,'$.description'),i.observed_at,i.observed_at,i.observed_at,
    json_extract(i.body,'$.rank'),''
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

UPDATE projects SET planning_rank=COALESCE((SELECT position FROM pm_wave_projects m
    WHERE m.wave_id=projects.wave_id AND m.project_id=projects.external_project_id),0);

-- Store the same editable Project content and relationship fields in every repository.
UPDATE projects SET (planning_provider_revision,planning_initiatives,planning_teams,project_prompt_context) = (
    SELECT json_extract(o.body,'$.revision'),json_extract(o.body,'$.initiative_ids'),
        json_extract(o.body,'$.team_ids'),
        '## Metric targets' || char(10) || json_extract(o.body,'$.metric_targets') || char(10) ||
        'workflow: ' || json_extract(o.body,'$.workflow') || char(10) || '## KRs' || char(10) ||
        COALESCE((SELECT group_concat('- [' || CASE json_extract(k.value,'$.holds') WHEN 1 THEN 'x' ELSE ' ' END || '] ' || json_extract(k.value,'$.text'),char(10))
            FROM json_each(o.body,'$.krs') k),'')
    FROM pm_projects o JOIN waves w ON w.id=projects.wave_id
    WHERE o.id=projects.external_project_id AND o.repo=w.repo AND o.provider='linear'
) WHERE EXISTS(SELECT 1 FROM pm_projects o JOIN waves w ON w.id=projects.wave_id
    WHERE o.id=projects.external_project_id AND o.repo=w.repo AND o.provider='linear');

-- Accepted planning state is retained independently of execution and provider inventory.
UPDATE tasks SET (planning_state,planning_completed,planning_completed_at,
    planning_provider_revision,planning_url,planning_branch_name,planning_team_id,
    planning_assignee,planning_rank,issue_identifier,issue_title,issue_description,pm_snapshot_synced_at) = (
    SELECT json_extract(i.body,'$.state'),json_extract(i.body,'$.completed'),
        json_extract(i.body,'$.completed_at'),json_extract(i.body,'$.revision'),
        json_extract(i.body,'$.url'),json_extract(i.body,'$.branch_name'),
        json_extract(i.body,'$.team_id'),json_extract(i.body,'$.assignee'),json_extract(i.body,'$.rank'),
        json_extract(i.body,'$.identifier'),json_extract(i.body,'$.name'),json_extract(i.body,'$.description'),i.observed_at
    FROM pm_items i JOIN projects p ON p.id=tasks.project_id
    JOIN waves w ON w.id=p.wave_id
    WHERE i.id=tasks.external_issue_id AND i.repo=w.repo AND i.provider='linear'
      AND i.needs_refresh=0
) WHERE EXISTS (
    SELECT 1 FROM pm_items i JOIN projects p ON p.id=tasks.project_id
    JOIN waves w ON w.id=p.wave_id
    WHERE i.id=tasks.external_issue_id AND i.repo=w.repo AND i.provider='linear'
      AND i.needs_refresh=0
);

-- The last saved local decision remains visible while delivery is uncertain.
UPDATE tasks SET planning_state=d.target,planning_completed=(d.target='completed'),
    planning_completed_at=CASE WHEN d.target='completed' THEN planning_completed_at ELSE NULL END
FROM task_state_deliveries d WHERE d.task_id=tasks.id AND d.settled=0
    AND d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=tasks.id);

UPDATE tasks SET planning_deleted_at=COALESCE(planning_deleted_at,updated_at,created_at)
WHERE EXISTS(SELECT 1 FROM pm_issue_changes c WHERE c.issue_id=tasks.external_issue_id AND c.removed=1);

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

CREATE TRIGGER store_revision_task_state_deliveries_insert AFTER INSERT ON task_state_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_state_deliveries_update AFTER UPDATE ON task_state_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_state_deliveries_delete AFTER DELETE ON task_state_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

-- Project edits and their delivery evidence commit together, including before mapping.
CREATE TABLE project_changes (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    field TEXT NOT NULL CHECK(field IN ('name','summary','workflow','krs','metric_targets','status')),
    value_json TEXT NOT NULL CHECK(json_valid(value_json)),
    base_json TEXT CHECK(base_json IS NULL OR json_valid(base_json)),
    conflict_json TEXT CHECK(conflict_json IS NULL OR json_valid(conflict_json)),
    acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged IN (0,1))
);
CREATE INDEX idx_project_changes_pending ON project_changes(project_id,field,seq);
CREATE TRIGGER store_revision_project_changes_insert AFTER INSERT ON project_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_project_changes_update AFTER UPDATE ON project_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_project_changes_delete AFTER DELETE ON project_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

-- Task edits and their delivery evidence commit together, including before mapping.
CREATE TABLE task_changes (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
    field TEXT NOT NULL CHECK(field IN ('name','description','assignee','rank','project_id')),
    value_json TEXT NOT NULL CHECK(json_valid(value_json)),
    base_json TEXT CHECK(base_json IS NULL OR json_valid(base_json)),
    conflict_json TEXT CHECK(conflict_json IS NULL OR json_valid(conflict_json)),
    acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged IN (0,1))
);
CREATE INDEX idx_task_changes_pending ON task_changes(task_id,field,seq);
CREATE TRIGGER store_revision_task_changes_insert AFTER INSERT ON task_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_changes_update AFTER UPDATE ON task_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_changes_delete AFTER DELETE ON task_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
