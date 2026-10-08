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
    repo TEXT PRIMARY KEY NOT NULL,
    id TEXT UNIQUE NOT NULL
);
INSERT INTO repository_plans(repo,id)
    SELECT repo, 'repo_' || lower(hex(randomblob(16))) FROM
        (SELECT DISTINCT repo FROM waves);
