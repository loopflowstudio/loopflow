-- draft: local_planning
PRAGMA legacy_alter_table = ON;

ALTER TABLE project_transitions ADD COLUMN local_plan_json TEXT;
ALTER TABLE project_transitions ADD COLUMN export_json TEXT CHECK(export_json IS NULL OR json_valid(export_json));
ALTER TABLE project_transitions ADD COLUMN export_attempted INTEGER NOT NULL DEFAULT 0;
ALTER TABLE project_transitions ADD COLUMN export_link_attempted INTEGER NOT NULL DEFAULT 0;
ALTER TABLE project_transitions ADD COLUMN export_error TEXT;
CREATE TABLE wave_workflows (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    PRIMARY KEY(wave_id,name)
);

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
    description TEXT NOT NULL,
    export_json TEXT CHECK(export_json IS NULL OR json_valid(export_json)),
    export_attempted INTEGER NOT NULL DEFAULT 0,
    export_error TEXT
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
    conflicting_comment_json TEXT CHECK(conflicting_comment_json IS NULL OR json_valid(conflicting_comment_json))
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

ALTER TABLE pm_projects ADD COLUMN task_order_json TEXT CHECK(task_order_json IS NULL OR json_valid(task_order_json));
-- Retain the last observed relative order at the released frontier. The next
-- complete acquisition adds new members and keeps omission unresolved.
UPDATE pm_projects AS observed SET task_order_json=(
    SELECT json_group_array(id) FROM (
        SELECT t.id FROM pm_items i JOIN tasks t ON t.external_issue_id=i.id
        JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id
        WHERE i.repo=observed.repo AND i.provider=observed.provider
          AND i.project_id=observed.id AND p.external_project_id=observed.id
          AND w.repo=observed.repo
        ORDER BY json_extract(i.body,'$.rank'),i.id
    )
);


-- Project edits and their delivery evidence commit together, including before mapping.
CREATE TABLE project_changes (
    order_effects_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(order_effects_json)),
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    field TEXT NOT NULL CHECK(field IN ('name','summary','workflow','krs','metric_targets','status','task_order')),
    value_json TEXT NOT NULL CHECK(json_valid(value_json)),
    base_json TEXT CHECK(base_json IS NULL OR json_valid(base_json)),
    conflict_json TEXT CHECK(conflict_json IS NULL OR json_valid(conflict_json)),
    attempted INTEGER NOT NULL DEFAULT 0 CHECK(attempted IN (0,1)),
    error TEXT,
    acknowledged_revision TEXT,
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
    field TEXT NOT NULL CHECK(field IN ('name','description','assignee','project_id','deleted')),
    value_json TEXT NOT NULL CHECK(json_valid(value_json)),
    base_json TEXT CHECK(base_json IS NULL OR json_valid(base_json)),
    conflict_json TEXT CHECK(conflict_json IS NULL OR json_valid(conflict_json)),
    attempted INTEGER NOT NULL DEFAULT 0 CHECK(attempted IN (0,1)),
    error TEXT,
    acknowledged_revision TEXT,
    acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged IN (0,1))
);
CREATE INDEX idx_task_changes_pending ON task_changes(task_id,field,seq);
CREATE TRIGGER store_revision_task_changes_insert AFTER INSERT ON task_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_changes_update AFTER UPDATE ON task_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_changes_delete AFTER DELETE ON task_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

-- Creation attempt errors are visible before a provider mapping exists.
CREATE TRIGGER store_revision_task_creation_intents_insert AFTER INSERT ON task_creation_intents
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_creation_intents_update AFTER UPDATE ON task_creation_intents
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_creation_intents_delete AFTER DELETE ON task_creation_intents
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

-- draft: optional_task_pr
-- Retire serial disposition authority while preserving its original evidence.
INSERT INTO task_events(task_id,kind_json,created_at)
SELECT task_id,json_object('kind','follow_through_conversion','reason',
 'Earlier delivery requested continuation: ' || COALESCE(next_slug,'unspecified scope')),unixepoch()
FROM task_prs p WHERE (after_merge='continue_task' OR next_slug IS NOT NULL)
AND sequence=(SELECT MAX(sequence) FROM task_prs WHERE task_id=p.task_id AND publication_requested_at IS NOT NULL);
CREATE TABLE task_prs_next (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 1),
    slug TEXT NOT NULL,
    branch TEXT NOT NULL,
    base_commit TEXT NOT NULL,
    publication_requested_at INTEGER,
    after_merge TEXT CHECK (after_merge IN ('continue_task', 'complete_task')),
    next_slug TEXT,
    github_number INTEGER CHECK (github_number > 0),
    github_url TEXT,
    merge_commit TEXT,
    abandoned_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    github_head_sha TEXT,
    ci_observation TEXT,
    parent_pr_id TEXT REFERENCES task_prs_next(id),
    github_observation TEXT,
    linear_attachment_id TEXT,
    linear_comment_id TEXT,
    linear_link_error TEXT,
    merge_mode TEXT CHECK (merge_mode IN ('user', 'auto')),
    merge_requested_at INTEGER,
    merge_head_sha TEXT CHECK (merge_head_sha IS NULL OR length(trim(merge_head_sha)) > 0),
    merged_at INTEGER,
    merge_tracking_complete INTEGER NOT NULL DEFAULT 0 CHECK(merge_tracking_complete IN (0,1)),
    repair_tracking_complete INTEGER NOT NULL DEFAULT 0 CHECK(repair_tracking_complete IN (0,1)),
    pr_title TEXT, pr_body TEXT, pr_copy_head_sha TEXT,
    UNIQUE (task_id, sequence),
    CHECK ((github_number IS NULL) = (github_url IS NULL)),
    CHECK (github_number IS NULL OR publication_requested_at IS NOT NULL),
    CHECK (after_merge != 'complete_task' OR next_slug IS NULL),
    CHECK (merge_commit IS NULL OR github_number IS NOT NULL),
    CHECK (merge_commit IS NULL OR abandoned_at IS NULL),
    CHECK ((merge_mode IS NULL) = (merge_requested_at IS NULL)),
    CHECK ((merge_mode IS NULL) = (merge_head_sha IS NULL)),
    CHECK (merge_mode IS NULL OR github_number IS NOT NULL),
    CHECK (merge_mode IS NULL OR merge_head_sha = github_head_sha)
);

INSERT INTO task_prs_next (id,task_id,sequence,slug,branch,base_commit,publication_requested_at,after_merge,next_slug,github_number,github_url,merge_commit,abandoned_at,created_at,updated_at,github_head_sha,ci_observation,parent_pr_id,github_observation,linear_attachment_id,linear_comment_id,linear_link_error,merge_mode,merge_requested_at,merge_head_sha,merged_at,merge_tracking_complete,repair_tracking_complete,pr_title,pr_body,pr_copy_head_sha) SELECT id,task_id,sequence,slug,branch,base_commit,publication_requested_at,after_merge,next_slug,github_number,github_url,merge_commit,abandoned_at,created_at,updated_at,github_head_sha,ci_observation,parent_pr_id,github_observation,linear_attachment_id,linear_comment_id,linear_link_error,merge_mode,merge_requested_at,merge_head_sha,merged_at,merge_tracking_complete,repair_tracking_complete,pr_title,pr_body,pr_copy_head_sha FROM task_prs;
DROP TRIGGER task_pr_repair_incidents_require_active_pr;
DROP TABLE task_prs;
ALTER TABLE task_prs_next RENAME TO task_prs;
CREATE TRIGGER task_pr_repair_incidents_require_active_pr
BEFORE INSERT ON task_pr_repair_incidents
WHEN EXISTS (
    SELECT 1
    FROM task_prs
    WHERE id = NEW.task_pr_id
      AND (merge_commit IS NOT NULL OR abandoned_at IS NOT NULL)
)
BEGIN
    SELECT RAISE(ABORT, 'repair incident requires an active Task PR');
END;


CREATE TRIGGER task_prs_enable_performance_tracking
AFTER INSERT ON task_prs
BEGIN
    UPDATE task_prs
    SET merge_tracking_complete = 1,
        repair_tracking_complete = 1
    WHERE id = NEW.id;
END;

CREATE TRIGGER task_prs_preserve_merged_at
BEFORE UPDATE OF merged_at ON task_prs
WHEN OLD.merged_at IS NOT NULL
 AND NEW.merged_at IS NOT OLD.merged_at
BEGIN
    SELECT RAISE(ABORT, 'Task PR merge evidence is immutable');
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
-- Checkout placement exists independently of publication. Retain every old PR id
-- because landing, stack and event history may still reference it.
ALTER TABLE tasks ADD COLUMN branch TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks ADD COLUMN base_commit TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks ADD COLUMN parent_pr_id TEXT REFERENCES task_prs(id);
UPDATE tasks SET
 branch=COALESCE((SELECT branch FROM task_prs WHERE task_id=tasks.id ORDER BY sequence DESC LIMIT 1), workspace_slug),
 base_commit=COALESCE((SELECT base_commit FROM task_prs WHERE task_id=tasks.id ORDER BY sequence DESC LIMIT 1), ''),
 parent_pr_id=(SELECT parent_pr_id FROM task_prs WHERE task_id=tasks.id ORDER BY sequence DESC LIMIT 1);
ALTER TABLE task_prs ADD COLUMN historical INTEGER NOT NULL DEFAULT 0 CHECK(historical IN (0,1));
UPDATE task_prs SET historical=1 WHERE publication_requested_at IS NULL
 OR sequence < (SELECT MAX(p.sequence) FROM task_prs p WHERE p.task_id=task_prs.task_id);
DROP INDEX IF EXISTS idx_task_prs_open;
CREATE UNIQUE INDEX idx_task_prs_current ON task_prs(task_id) WHERE historical=0;
CREATE UNIQUE INDEX idx_task_prs_current_branch ON task_prs(branch) WHERE historical=0;
CREATE TRIGGER task_pr_history_immutable BEFORE UPDATE ON task_prs WHEN OLD.historical=1
BEGIN SELECT RAISE(ABORT, 'historical Task PRs are read-only'); END;
CREATE TRIGGER task_pr_history_no_delete BEFORE DELETE ON task_prs WHEN OLD.historical=1
BEGIN SELECT RAISE(ABORT, 'historical Task PRs are read-only'); END;
CREATE TRIGGER task_pr_history_no_insert BEFORE INSERT ON task_prs WHEN NEW.historical=1
BEGIN SELECT RAISE(ABORT, 'only migration can create historical Task PRs'); END;
CREATE TRIGGER task_pr_history_no_mark BEFORE UPDATE OF historical ON task_prs
WHEN NEW.historical<>OLD.historical
BEGIN SELECT RAISE(ABORT, 'only migration can mark historical Task PRs'); END;
-- Task owns placement. PR fields are the validated mirror retained by the PR API.
CREATE TRIGGER task_pr_placement_insert BEFORE INSERT ON task_prs WHEN NEW.historical=0
AND NOT EXISTS(SELECT 1 FROM tasks t WHERE t.id=NEW.task_id AND t.branch=NEW.branch
 AND t.base_commit=NEW.base_commit AND t.parent_pr_id IS NEW.parent_pr_id)
BEGIN SELECT RAISE(ABORT, 'Task PR placement differs from Task'); END;
CREATE TRIGGER task_placement_to_pr AFTER UPDATE OF branch,base_commit,parent_pr_id ON tasks
BEGIN
 UPDATE task_prs SET branch=NEW.branch, base_commit=NEW.base_commit,
 parent_pr_id=NEW.parent_pr_id WHERE task_id=NEW.id AND historical=0;
END;
CREATE TRIGGER task_pr_placement_update BEFORE UPDATE OF branch,base_commit,parent_pr_id ON task_prs
WHEN NEW.historical=0 AND NOT EXISTS(SELECT 1 FROM tasks t WHERE t.id=NEW.task_id
 AND t.branch=NEW.branch AND t.base_commit=NEW.base_commit AND t.parent_pr_id IS NEW.parent_pr_id)
BEGIN SELECT RAISE(ABORT, 'Task PR placement differs from Task'); END;

-- Completion is owned by local planning; retain only the durable end trigger here.
ALTER TABLE tasks ADD COLUMN completion_request INTEGER;
ALTER TABLE tasks ADD COLUMN completion_error TEXT;
UPDATE tasks SET planning_completed=1, planning_state='completed',
 planning_completed_at=COALESCE(planning_completed_at,strftime('%Y-%m-%dT%H:%M:%SZ',updated_at,'unixepoch'))
WHERE abandoned_at IS NULL AND planning_provider_revision IS NULL AND EXISTS(
 SELECT 1 FROM task_workflows WHERE task_id=tasks.id AND node='end' AND edge IS NULL);
-- Pending provider delivery was migrated by local_planning; it remains independent
-- of Workflow arrival and is never replayed as a new completion request.

ALTER TABLE tasks ADD COLUMN planning_due_date TEXT;
ALTER TABLE task_creation_intents ADD COLUMN due_date TEXT;
UPDATE tasks SET planning_due_date=(SELECT json_extract(i.body,'$.due_date') FROM pm_items i
 JOIN projects p ON p.id=tasks.project_id JOIN waves w ON w.id=p.wave_id
 WHERE i.id=tasks.external_issue_id AND i.repo=w.repo AND i.provider='linear');

-- draft: agent_process
-- One process inventory. Backfill directly from the released Session shape;
-- history payloads and native thread identity remain unchanged.
ALTER TABLE processes ADD COLUMN kind TEXT NOT NULL DEFAULT 'lf' CHECK(kind IN ('lf','agent'));
ALTER TABLE processes ADD COLUMN agent_session_id TEXT REFERENCES agent_sessions(id);
ALTER TABLE processes ADD COLUMN os_started_at INTEGER;
ALTER TABLE processes ADD COLUMN endpoint TEXT;
ALTER TABLE processes ADD COLUMN agent_provider TEXT;
ALTER TABLE processes ADD COLUMN agent_interactive INTEGER CHECK(agent_interactive IN (0,1));
ALTER TABLE processes ADD COLUMN caller_agent_process_id TEXT;
ALTER TABLE processes ADD COLUMN attached_lf_process_id TEXT;
ALTER TABLE processes ADD COLUMN attachment_token TEXT;
ALTER TABLE processes ADD COLUMN attachment_exit_seq INTEGER REFERENCES session_events(seq);
ALTER TABLE processes ADD COLUMN spawn_state TEXT CHECK(spawn_state IN ('reserved','spawn_requested','spawn_failed','exited'));
ALTER TABLE agent_sessions ADD COLUMN agent_process_id TEXT REFERENCES processes(lfid);
CREATE TEMP TABLE agent_process_backfill AS SELECT id AS session_id,
    (lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))),2) || '-8' ||
    substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6)))) AS lfid,
    CASE WHEN driver_generation>0 THEN
    (lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))),2) || '-8' ||
    substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6)))) END AS attachment_token
FROM agent_sessions WHERE provider_generation>0 OR provider_pid IS NOT NULL
    OR driver_generation>0 OR driver_process_lfid IS NOT NULL;
INSERT INTO processes(lfid,kind,trace_id,parent_process_lfid,command,repo,cwd,started_at,
    pid,os_started_at,agent_session_id,endpoint,agent_provider,agent_interactive,
    attached_lf_process_id,attachment_token,attachment_exit_seq,spawn_state,completed_at)
SELECT b.lfid,'agent',COALESCE(p.trace_id,b.lfid),s.provider_process_lfid,
    s.provider,s.repo,s.cwd,COALESCE(s.provider_started_at,s.created_at),
    s.provider_pid,s.provider_started_at,s.id,s.provider_endpoint,s.provider,s.interactive,
    s.driver_process_lfid,b.attachment_token,
    (SELECT seq FROM session_events e WHERE e.session_id=s.id AND e.kind='observed'
        AND e.receipt_key='driver:'||(s.driver_generation-1)||':exit'),
    (SELECT json_extract(e.payload,'$.phase') FROM session_events e
     WHERE e.session_id=s.id AND e.receipt_key IN (
        'provider:'||s.provider_generation||':reserved',
        'provider:'||s.provider_generation||':spawn_requested',
        'provider:'||s.provider_generation||':spawn_failed',
        'provider:'||s.provider_generation||':exited') ORDER BY e.seq DESC LIMIT 1),
    (SELECT e.observed_at FROM session_events e WHERE e.session_id=s.id AND e.receipt_key IN (
        'provider:'||s.provider_generation||':spawn_failed',
        'provider:'||s.provider_generation||':exited') ORDER BY e.seq DESC LIMIT 1)
FROM agent_sessions s JOIN agent_process_backfill b ON b.session_id=s.id
LEFT JOIN processes p ON p.lfid=s.provider_process_lfid;
UPDATE agent_sessions SET agent_process_id=(SELECT lfid FROM agent_process_backfill b WHERE b.session_id=agent_sessions.id);
ALTER TABLE session_activity ADD COLUMN attachment_token TEXT;
UPDATE session_activity SET attachment_token=(
    SELECT b.attachment_token FROM agent_process_backfill b JOIN agent_sessions s ON s.id=b.session_id
    WHERE s.id=session_activity.session_id AND s.driver_generation=session_activity.driver_generation
);
-- The AgentProcess identity replaces the numeric provider generation. Only a
-- Session's current generation has a record; earlier ones stay unknown. The
-- released generation columns on processes and session_events remain as
-- unread history: nothing maps them, and dropping one rewrites every event.
ALTER TABLE session_activity ADD COLUMN agent_process_id TEXT;
UPDATE session_activity SET agent_process_id=(
    SELECT b.lfid FROM agent_process_backfill b JOIN agent_sessions s ON s.id=b.session_id
    WHERE s.id=session_activity.session_id AND s.provider_generation=session_activity.provider_generation
);
ALTER TABLE session_events ADD COLUMN agent_process_id TEXT;
UPDATE session_events SET agent_process_id=(
    SELECT b.lfid FROM agent_process_backfill b JOIN agent_sessions s ON s.id=b.session_id
    WHERE s.id=session_events.session_id AND s.provider_generation=session_events.provider_generation
) WHERE kind='started' AND provider_generation IS NOT NULL
    AND session_id IN (SELECT session_id FROM agent_process_backfill);
UPDATE processes SET caller_agent_process_id=(
    SELECT b.lfid FROM agent_process_backfill b JOIN agent_sessions s ON s.id=b.session_id
    WHERE s.id=processes.caller_session_id AND s.provider_generation=processes.caller_provider_generation
) WHERE caller_provider_generation IS NOT NULL;
DROP TABLE agent_process_backfill;
-- Preserve conflicting PID/birth observations separately; never deduplicate them
-- into signal authority or invent terminal evidence for uncertain history.
CREATE INDEX agent_process_session ON processes(agent_session_id) WHERE kind='agent';
CREATE INDEX agent_process_attachment ON processes(attached_lf_process_id) WHERE kind='agent';
DROP INDEX session_driver_process;
DROP INDEX session_provider_pid;
DROP INDEX session_unknown_engine_origin;
DROP INDEX session_driver_exit;
CREATE INDEX session_observation_receipt ON session_events(session_id,receipt_key) WHERE kind='observed';
DROP TRIGGER store_revision_session_activity_update;
ALTER TABLE session_activity DROP COLUMN driver_generation;
ALTER TABLE session_activity DROP COLUMN provider_generation;
ALTER TABLE agent_sessions DROP COLUMN driver_generation;
ALTER TABLE agent_sessions DROP COLUMN driver_process_lfid;
ALTER TABLE agent_sessions DROP COLUMN provider_pid;
ALTER TABLE agent_sessions DROP COLUMN provider_started_at;
ALTER TABLE agent_sessions DROP COLUMN provider_endpoint;
ALTER TABLE agent_sessions DROP COLUMN provider_generation;
ALTER TABLE agent_sessions DROP COLUMN provider_process_lfid;
CREATE TRIGGER store_revision_session_activity_update AFTER UPDATE ON session_activity
WHEN NEW.attachment_token IS NOT OLD.attachment_token
    OR NEW.agent_process_id IS NOT OLD.agent_process_id
    OR NEW.program_status IS NOT OLD.program_status
    OR (NEW.pending_input > 0) IS NOT (OLD.pending_input > 0)
    OR (NEW.open_tools = 0) IS NOT (OLD.open_tools = 0)
    OR NEW.yielded IS NOT OLD.yielded
    OR (OLD.open_tools = 0 AND NEW.observed_at - OLD.observed_at >= 120)
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_agent_process_update AFTER UPDATE ON processes
WHEN NEW.kind='agent'
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;

-- draft: lf_process_ids
-- An LfProcess's identity is its id; every reference names its role. SQLite
-- rewrites surviving indexes, triggers and foreign keys during each rename.
-- Rows, IDs, outcomes and history payloads remain unchanged.
ALTER TABLE processes RENAME COLUMN lfid TO id;
ALTER TABLE processes RENAME COLUMN parent_process_lfid TO parent_lf_process_id;
ALTER TABLE waves RENAME COLUMN project_activation_process_lfid TO project_activation_lf_process_id;
ALTER TABLE ci_incidents RENAME COLUMN repair_process_lfid TO repair_lf_process_id;
ALTER TABLE session_events RENAME COLUMN process_lfid TO lf_process_id;
ALTER TABLE flow_processes RENAME COLUMN process_lfid TO lf_process_id;
ALTER TABLE flow_process_steps RENAME COLUMN flow_process_lfid TO flow_lf_process_id;
ALTER TABLE flow_process_steps RENAME COLUMN process_lfid TO lf_process_id;
ALTER TABLE task_workflows RENAME COLUMN process_lfid TO lf_process_id;
ALTER TABLE task_workflow_moves RENAME COLUMN process_lfid TO lf_process_id;
DROP TRIGGER validate_flow_process_step;
CREATE TRIGGER validate_flow_process_step BEFORE INSERT ON flow_process_steps BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM processes step
        WHERE step.id=NEW.lf_process_id AND step.parent_lf_process_id=NEW.flow_lf_process_id
    ) THEN RAISE(ABORT,'A Flow step is a process its Flow process started') END;
END;
-- Attachment exits recorded under the released receipt prefix keep their
-- generation and payload; only the key's prefix changes.
UPDATE session_events SET receipt_key='attachment:'||substr(receipt_key,8)
WHERE kind='observed' AND receipt_key>='driver:' AND receipt_key<'driver;';
