-- Stable Tasks no longer retry the Session-era Linear reopen operation.
UPDATE tasks
SET pm_writeback_json = '{"state":"current"}'
WHERE json_extract(pm_writeback_json, '$.state') = 'pending'
  AND json_extract(pm_writeback_json, '$.operation') = 'reopen_task';
