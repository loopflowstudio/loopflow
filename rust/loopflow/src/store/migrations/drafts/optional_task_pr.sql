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

-- Completion is independent of Workflow position. Seed only from released facts.
ALTER TABLE tasks ADD COLUMN completed_at INTEGER;
ALTER TABLE tasks ADD COLUMN completion_request INTEGER;
ALTER TABLE tasks ADD COLUMN completion_error TEXT;
UPDATE tasks SET completed_at=COALESCE(
 (SELECT MAX(created_at) FROM task_events WHERE task_id=tasks.id AND json_extract(kind_json,'$.kind')='completed'),updated_at)
WHERE abandoned_at IS NULL AND EXISTS(
 SELECT 1 FROM task_workflows WHERE task_id=tasks.id AND node='end' AND edge IS NULL);
-- Preserve unfinished provider writebacks from already completed historical Tasks.
INSERT INTO task_events(task_id,kind_json,created_at)
SELECT id,json_object('kind','completion_requested','reason','Retained completion writeback'),unixepoch()
FROM tasks WHERE json_extract(pm_writeback_json,'$.state')='pending';
UPDATE tasks SET completion_request=(SELECT MAX(id) FROM task_events WHERE task_id=tasks.id
 AND json_extract(kind_json,'$.kind')='completion_requested')
WHERE json_extract(pm_writeback_json,'$.state')='pending';
-- Previously conflicting accepted provider completion now owns status without
-- moving an active Workflow or settling any Process.
UPDATE tasks SET completed_at=COALESCE(completed_at,(
 SELECT i.observed_at FROM pm_items i JOIN projects p ON p.id=tasks.project_id
 JOIN waves w ON w.id=p.wave_id
 WHERE i.id=tasks.external_issue_id AND i.repo=w.repo AND i.provider='linear'
 AND i.needs_refresh=0 AND (json_extract(i.body,'$.state')='completed'
 OR (json_extract(i.body,'$.state') IS NULL AND json_extract(i.body,'$.completed')=1))
)) WHERE abandoned_at IS NULL;
