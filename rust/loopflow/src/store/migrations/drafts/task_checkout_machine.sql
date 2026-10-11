-- depends_on: local_planning, planning_peers

ALTER TABLE tasks ADD COLUMN checkout_machine_id TEXT REFERENCES machines(id);
ALTER TABLE tasks ADD COLUMN admission_authority TEXT CHECK(admission_authority IS NULL OR json_valid(admission_authority));

-- The creating Machine arbitrates once. Other Machines may retain its positive
-- receipt, but no claim participates in planning merge or expires on absence.
-- Keep claims after a Task is removed; reimport cannot reopen first start.
CREATE TABLE task_admissions (
    task_id TEXT PRIMARY KEY,
    machine_id TEXT NOT NULL
);
CREATE TRIGGER task_admissions_immutable BEFORE UPDATE ON task_admissions
BEGIN SELECT RAISE(ABORT, 'Task admission cannot transfer execution'); END;
CREATE TRIGGER task_admissions_retained BEFORE DELETE ON task_admissions
BEGIN SELECT RAISE(ABORT, 'Task admission cannot be discarded'); END;
CREATE TRIGGER store_revision_task_admissions_insert AFTER INSERT ON task_admissions
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

-- Preserve the only recorded location evidence. Missing placement stays unknown.
UPDATE tasks SET checkout_machine_id = (
    SELECT machine_id FROM work_placements WHERE task_id = tasks.id
) WHERE worktree != '';

-- Historical assignments may be explicit or copied. Never infer that distinction
-- from equality with a parent's Machine. New children resolve their ancestry.
-- A portable assignment may name a Machine with no local connection. Keep its
-- identity without manufacturing a route; execution locations still require one.
CREATE TABLE delegated_placements (
    wave_id TEXT REFERENCES waves(id) ON DELETE CASCADE,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    machine_id TEXT NOT NULL,
    placed_at INTEGER NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    provenance TEXT NOT NULL DEFAULT 'legacy' CHECK (provenance IN ('explicit', 'legacy')),
    CHECK ((wave_id IS NOT NULL) + (project_id IS NOT NULL) + (task_id IS NOT NULL) = 1),
    UNIQUE (wave_id), UNIQUE (project_id), UNIQUE (task_id)
);
INSERT INTO delegated_placements
    SELECT wave_id,project_id,task_id,machine_id,placed_at,enabled,'legacy' FROM work_placements;
DROP TABLE work_placements;
ALTER TABLE delegated_placements RENAME TO work_placements;
CREATE INDEX idx_work_placements_machine ON work_placements(machine_id,placed_at);

-- The selected plan's repository identity is independent of its local locator.
-- Peer adoption supplies the same id explicitly; a code remote implies nothing.
CREATE TABLE repository_plans (
    id TEXT PRIMARY KEY NOT NULL,
    repo TEXT NOT NULL,
    selected INTEGER NOT NULL CHECK (selected IN (0, 1))
);
CREATE UNIQUE INDEX repository_selected_plan ON repository_plans(repo) WHERE selected=1;
INSERT INTO repository_plans(repo,id,selected)
    SELECT repo, 'repo_' || lower(hex(randomblob(16))), 1 FROM
        (SELECT DISTINCT repo FROM waves);

CREATE TRIGGER store_revision_repository_plans_insert AFTER INSERT ON repository_plans
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_repository_plans_update AFTER UPDATE ON repository_plans
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_repository_plans_delete AFTER DELETE ON repository_plans
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_work_placements_insert AFTER INSERT ON work_placements
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_work_placements_update AFTER UPDATE ON work_placements
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_work_placements_delete AFTER DELETE ON work_placements
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
