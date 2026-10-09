-- depends_on: local_planning

ALTER TABLE tasks ADD COLUMN checkout_machine_id TEXT REFERENCES machines(id);

-- Preserve the only recorded location evidence. Missing placement stays unknown.
UPDATE tasks SET checkout_machine_id = (
    SELECT machine_id FROM work_placements WHERE task_id = tasks.id
) WHERE worktree != '';

-- Historical assignments may be explicit or copied. Never infer that distinction
-- from equality with a parent's Machine. New children resolve their ancestry.
ALTER TABLE work_placements ADD COLUMN provenance TEXT NOT NULL DEFAULT 'legacy'
    CHECK (provenance IN ('explicit', 'legacy'));

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
