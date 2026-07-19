-- name: retire_obsolete_pm_reopen_writebacks
-- id: 3152c8349e624cc5ab4ab08c9b50c268
-- depends_on: delete_sessions
-- Stable Tasks no longer retry the Session-era Linear reopen operation.
UPDATE tasks
SET pm_writeback_json = '{"state":"current"}'
WHERE json_extract(pm_writeback_json, '$.state') = 'pending'
  AND json_extract(pm_writeback_json, '$.operation') = 'reopen_task';
