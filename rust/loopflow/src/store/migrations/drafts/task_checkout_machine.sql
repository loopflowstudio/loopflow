-- depends_on: rename_home_to_machine

ALTER TABLE tasks ADD COLUMN checkout_machine_id TEXT REFERENCES machines(id);

-- Preserve the only recorded location evidence. Missing placement stays unknown.
UPDATE tasks SET checkout_machine_id = (
    SELECT machine_id FROM work_placements WHERE task_id = tasks.id
) WHERE worktree != '';
