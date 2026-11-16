-- Project and Task are the durable product records; Run is their sole executor.

ALTER TABLE projects ADD COLUMN project_slug TEXT;
ALTER TABLE projects ADD COLUMN project_name TEXT;
ALTER TABLE projects ADD COLUMN project_prompt_context TEXT;
ALTER TABLE projects ADD COLUMN pm_snapshot_synced_at INTEGER;
ALTER TABLE projects ADD COLUMN iteration INTEGER;
ALTER TABLE projects ADD COLUMN observation_cursor INTEGER;
ALTER TABLE projects ADD COLUMN last_state_fingerprint TEXT;
ALTER TABLE projects ADD COLUMN agent TEXT;
ALTER TABLE projects ADD COLUMN provider TEXT;
ALTER TABLE projects ADD COLUMN provider_session_id TEXT;
ALTER TABLE projects ADD COLUMN abandon_requested_at INTEGER;
ALTER TABLE projects ADD COLUMN abandon_reason TEXT;
ALTER TABLE projects ADD COLUMN updated_at INTEGER;

UPDATE projects
SET (project_slug, project_name, project_prompt_context,
     pm_snapshot_synced_at,
     iteration, observation_cursor, last_state_fingerprint,
     agent, provider, provider_session_id, abandon_requested_at,
     abandon_reason, updated_at) = (
    SELECT s.project_slug, s.project_name, s.project_prompt_context,
           s.pm_snapshot_synced_at,
           s.iteration, s.observation_cursor, s.last_state_fingerprint,
           s.agent, s.provider, s.provider_session_id,
           s.abandon_requested_at, s.abandon_reason, s.updated_at
    FROM project_sessions s
    WHERE s.project_id=projects.external_project_id
    ORDER BY s.created_at DESC, s.id DESC LIMIT 1
);

CREATE INDEX idx_projects_wave_updated ON projects(wave_id, updated_at DESC);

ALTER TABLE tasks ADD COLUMN issue_title TEXT;
ALTER TABLE tasks ADD COLUMN issue_description TEXT;
ALTER TABLE tasks ADD COLUMN pm_snapshot_synced_at INTEGER;
ALTER TABLE tasks ADD COLUMN pm_writeback_json TEXT;
ALTER TABLE tasks ADD COLUMN worktree TEXT;
ALTER TABLE tasks ADD COLUMN workspace_slug TEXT;
ALTER TABLE tasks ADD COLUMN agent TEXT;
ALTER TABLE tasks ADD COLUMN provider TEXT;
ALTER TABLE tasks ADD COLUMN provider_session_id TEXT;
ALTER TABLE tasks ADD COLUMN abandon_requested_at INTEGER;
ALTER TABLE tasks ADD COLUMN abandon_reason TEXT;
ALTER TABLE tasks ADD COLUMN iterate_flow TEXT;
ALTER TABLE tasks ADD COLUMN iterate_interaction_policy TEXT;
ALTER TABLE tasks ADD COLUMN phase_cursor INTEGER;
ALTER TABLE tasks ADD COLUMN phase_iteration INTEGER;
ALTER TABLE tasks ADD COLUMN kickoff_flow TEXT;
ALTER TABLE tasks ADD COLUMN kickoff_interaction_policy TEXT;
ALTER TABLE tasks ADD COLUMN gate_flow TEXT;
ALTER TABLE tasks ADD COLUMN gate_interaction_policy TEXT;
ALTER TABLE tasks ADD COLUMN lifecycle_phase TEXT;
ALTER TABLE tasks ADD COLUMN phase_epoch INTEGER;
ALTER TABLE tasks ADD COLUMN gate_cycle INTEGER;
ALTER TABLE tasks ADD COLUMN gate_proposal_json TEXT;
ALTER TABLE tasks ADD COLUMN updated_at INTEGER;

UPDATE tasks
SET (issue_identifier, issue_title, issue_description,
     pm_snapshot_synced_at, pm_writeback_json,
     worktree, workspace_slug, agent, provider,
     provider_session_id, abandon_requested_at, abandon_reason,
     iterate_flow, iterate_interaction_policy, phase_cursor,
     phase_iteration, kickoff_flow, kickoff_interaction_policy,
     gate_flow, gate_interaction_policy, lifecycle_phase, phase_epoch,
     gate_cycle, gate_proposal_json, updated_at) = (
    SELECT s.issue_identifier, s.issue_title, s.issue_description,
           s.pm_snapshot_synced_at, s.pm_writeback_json,
           s.worktree, s.workspace_slug,
           s.agent, s.provider, s.provider_session_id,
           s.abandon_requested_at, s.abandon_reason, s.iterate_flow,
           s.iterate_interaction_policy, s.phase_cursor, s.phase_iteration,
           s.kickoff_flow, s.kickoff_interaction_policy, s.gate_flow,
           s.gate_interaction_policy, s.lifecycle_phase, s.phase_epoch,
           s.gate_cycle, s.gate_proposal_json, s.updated_at
    FROM task_sessions s
    WHERE s.issue_id=tasks.external_issue_id
    ORDER BY s.created_at DESC, s.id DESC LIMIT 1
);

CREATE UNIQUE INDEX idx_tasks_issue_identifier ON tasks(issue_identifier);
CREATE UNIQUE INDEX idx_tasks_worktree ON tasks(worktree);
CREATE INDEX idx_tasks_updated ON tasks(updated_at DESC);

CREATE TABLE work_placements (
    wave_id TEXT REFERENCES waves(id) ON DELETE CASCADE,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    home_id TEXT NOT NULL REFERENCES homes(id) ON DELETE RESTRICT,
    placed_at INTEGER NOT NULL,
    CHECK (
        (wave_id IS NOT NULL) +
        (project_id IS NOT NULL) +
        (task_id IS NOT NULL) = 1
    ),
    UNIQUE (wave_id),
    UNIQUE (project_id),
    UNIQUE (task_id)
);
CREATE INDEX idx_work_placements_home
    ON work_placements(home_id, placed_at);

-- Run authority follows stable Work, not the Session row that first opened its
-- Epoch. Re-key historical Runs before deleting that compatibility identity.
UPDATE runs
SET source_id = (
    SELECT projects.id
    FROM project_sessions
    JOIN projects ON projects.external_project_id=project_sessions.project_id
    WHERE project_sessions.id=runs.source_id
)
WHERE source_kind='project';

UPDATE runs
SET source_id = (
    SELECT tasks.id
    FROM task_sessions
    JOIN tasks ON tasks.external_issue_id=task_sessions.issue_id
    WHERE task_sessions.id=runs.source_id
)
WHERE source_kind='task';

INSERT INTO work_placements (wave_id, home_id, placed_at)
SELECT waves.id, homes.id, waves.created_at
FROM waves
JOIN homes ON homes.route='local';

INSERT INTO work_placements (project_id, home_id, placed_at)
SELECT projects.id, work_placements.home_id, projects.created_at
FROM projects
JOIN work_placements ON work_placements.wave_id=projects.wave_id;

INSERT INTO work_placements (task_id, home_id, placed_at)
SELECT tasks.id, work_placements.home_id, tasks.created_at
FROM tasks
JOIN work_placements ON work_placements.project_id=tasks.project_id;

CREATE TABLE task_prs_next (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 1),
    slug TEXT NOT NULL,
    branch TEXT NOT NULL UNIQUE,
    base_commit TEXT NOT NULL,
    publication_requested_at INTEGER,
    after_merge TEXT CHECK (after_merge IN ('review', 'complete_task')),
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
    UNIQUE (task_id, sequence),
    CHECK ((publication_requested_at IS NULL) = (after_merge IS NULL)),
    CHECK ((github_number IS NULL) = (github_url IS NULL)),
    CHECK (github_number IS NULL OR publication_requested_at IS NOT NULL),
    CHECK (after_merge != 'complete_task' OR next_slug IS NULL),
    CHECK (merge_commit IS NULL OR github_number IS NOT NULL),
    CHECK (merge_commit IS NULL OR abandoned_at IS NULL)
);

INSERT INTO task_prs_next
SELECT pr.id, t.id,
       row_number() OVER (PARTITION BY t.id ORDER BY pr.created_at, pr.id),
       pr.slug, pr.branch, pr.base_commit, pr.publication_requested_at,
       pr.after_merge, pr.next_slug, pr.github_number, pr.github_url,
       pr.merge_commit, pr.abandoned_at, pr.created_at, pr.updated_at,
       pr.github_head_sha, pr.ci_observation, pr.parent_pr_id,
       pr.github_observation, pr.linear_attachment_id, pr.linear_comment_id,
       pr.linear_link_error
FROM task_prs pr
JOIN task_sessions s ON s.id=pr.task_session_id
JOIN tasks t ON t.external_issue_id=s.issue_id;

DROP TABLE task_prs;
ALTER TABLE task_prs_next RENAME TO task_prs;
CREATE UNIQUE INDEX idx_task_prs_open
    ON task_prs(task_id) WHERE merge_commit IS NULL AND abandoned_at IS NULL;

CREATE TABLE ci_incidents_next (
    identity TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    pr_id TEXT NOT NULL REFERENCES task_prs(id) ON DELETE CASCADE,
    repo TEXT NOT NULL,
    pr_number INTEGER NOT NULL CHECK (pr_number > 0),
    failed_head_sha TEXT NOT NULL,
    failure_set_json TEXT NOT NULL,
    provider_completed_at INTEGER,
    poll_observed_at INTEGER,
    webhook_received_at INTEGER,
    claimed_run_id TEXT REFERENCES runs(id) ON DELETE RESTRICT,
    responded_at INTEGER,
    green_at INTEGER,
    merged_at INTEGER,
    blocked_at INTEGER,
    blocked_reason TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    repaired_head_sha TEXT,
    CHECK (poll_observed_at IS NOT NULL OR webhook_received_at IS NOT NULL),
    CHECK ((blocked_at IS NULL) = (blocked_reason IS NULL))
);

INSERT INTO ci_incidents_next
SELECT ci.identity, t.id, ci.pr_id, ci.repo, ci.pr_number,
       ci.failed_head_sha, ci.failure_set_json, ci.provider_completed_at,
       ci.poll_observed_at, ci.webhook_received_at, ci.claimed_run_id,
       ci.responded_at, ci.green_at, ci.merged_at, ci.blocked_at,
       ci.blocked_reason, ci.created_at, ci.updated_at, ci.repaired_head_sha
FROM ci_incidents ci
JOIN task_sessions s ON s.id=ci.task_session_id
JOIN tasks t ON t.external_issue_id=s.issue_id;

DROP TABLE ci_incidents;
ALTER TABLE ci_incidents_next RENAME TO ci_incidents;
CREATE INDEX idx_ci_incidents_observed
    ON ci_incidents(poll_observed_at, webhook_received_at);
CREATE INDEX idx_ci_incidents_pr ON ci_incidents(pr_id, created_at);
CREATE INDEX idx_ci_incidents_open ON ci_incidents(green_at, merged_at, updated_at);
CREATE INDEX idx_ci_incidents_run ON ci_incidents(claimed_run_id, updated_at);

CREATE TABLE task_linear_observations_next (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    last_revision TEXT NOT NULL,
    last_title TEXT NOT NULL,
    last_description TEXT NOT NULL,
    last_success_at INTEGER NOT NULL,
    degraded_reason TEXT,
    updated_at INTEGER NOT NULL
);
INSERT OR REPLACE INTO task_linear_observations_next
SELECT t.id, o.last_revision, o.last_title, o.last_description,
       o.last_success_at, o.degraded_reason, o.updated_at
FROM task_linear_observations o
JOIN task_sessions s ON s.id=o.session_id
JOIN tasks t ON t.external_issue_id=s.issue_id
ORDER BY o.updated_at;
DROP TABLE task_linear_observations;
ALTER TABLE task_linear_observations_next RENAME TO task_linear_observations;

CREATE TABLE task_linear_ingested_comments_next (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    comment_id TEXT NOT NULL,
    ingested_at INTEGER NOT NULL,
    PRIMARY KEY (task_id, comment_id)
);
INSERT OR IGNORE INTO task_linear_ingested_comments_next
SELECT t.id, c.comment_id, c.ingested_at
FROM task_linear_ingested_comments c
JOIN task_sessions s ON s.id=c.session_id
JOIN tasks t ON t.external_issue_id=s.issue_id;
DROP TABLE task_linear_ingested_comments;
ALTER TABLE task_linear_ingested_comments_next
    RENAME TO task_linear_ingested_comments;

-- Events are wake hints, not durable truth. Reconstruct them from current Work.
DROP TABLE task_events;
CREATE TABLE task_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    kind_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_task_events_task ON task_events(task_id, id);

DROP TABLE project_events;
CREATE TABLE project_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_project_events_project ON project_events(project_id, id);

DELETE FROM observation_outbox;
DROP TABLE task_sessions;
DROP TABLE project_sessions;
