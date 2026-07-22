-- name: task_performance_authority
-- id: 854929ca56179897f90a6146600d86d6
-- depends_on:

-- One cutover owns the meaning of absence for lifecycle performance evidence.
-- Historical terminal PRs remain uncovered. Triggers cover every PR created
-- after the migration even when an older installed writer omits new columns.
CREATE TABLE performance_evidence_authority (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    started_at INTEGER NOT NULL
);
INSERT INTO performance_evidence_authority (singleton, started_at)
VALUES (1, unixepoch());

ALTER TABLE runs ADD COLUMN first_material_at INTEGER;

CREATE TRIGGER runs_preserve_first_material
BEFORE UPDATE OF first_material_at ON runs
WHEN OLD.first_material_at IS NOT NULL
 AND NEW.first_material_at IS NOT OLD.first_material_at
BEGIN
    SELECT RAISE(ABORT, 'Run first material evidence is immutable');
END;

ALTER TABLE task_prs ADD COLUMN merged_at INTEGER;
ALTER TABLE task_prs
ADD COLUMN merge_tracking_complete INTEGER NOT NULL DEFAULT 0
CHECK (merge_tracking_complete IN (0, 1));
ALTER TABLE task_prs
ADD COLUMN repair_tracking_complete INTEGER NOT NULL DEFAULT 0
CHECK (repair_tracking_complete IN (0, 1));

-- A PR still active at cutover has not reached its merge boundary, so the new
-- writer can cover its future GitHub timestamp. Its repair history may already
-- be partial, so repair tracking intentionally remains incomplete.
UPDATE task_prs
SET merge_tracking_complete = 1
WHERE merge_commit IS NULL AND abandoned_at IS NULL;

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

CREATE TABLE task_pr_repair_incidents (
    task_pr_id TEXT NOT NULL REFERENCES task_prs(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (
        kind IN ('avoidable_rebase_agent', 'manual_git_repair')
    ),
    occurred_at INTEGER NOT NULL,
    PRIMARY KEY (task_pr_id, kind)
);

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

CREATE TRIGGER task_pr_repair_incidents_are_immutable
BEFORE UPDATE ON task_pr_repair_incidents
BEGIN
    SELECT RAISE(ABORT, 'Task PR repair incidents are immutable');
END;
