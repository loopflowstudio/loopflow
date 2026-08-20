-- draft: repair_legacy_task_flow
-- `task` was the historical default loop flow. `slice` is its current
-- replacement; change only that retired default and preserve explicit flows.
UPDATE tasks
SET iterate_flow = 'slice'
WHERE iterate_flow = 'task';
