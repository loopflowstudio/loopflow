-- name: repair_legacy_task_flow
-- id: 6e8fbbdbbfa6ef69991bac7f448c25f1
-- depends_on:
-- `task` was the historical default loop flow. `slice` is its current
-- replacement; change only that retired default and preserve explicit flows.
UPDATE tasks
SET iterate_flow = 'slice'
WHERE iterate_flow = 'task';
