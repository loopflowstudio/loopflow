
-- CI repair retains its existing per-Task hold. Saved Flow scheduling is retired.
ALTER TABLE tasks DROP COLUMN automation_exec_id;
ALTER TABLE tasks DROP COLUMN automation_retry_key;
ALTER TABLE tasks DROP COLUMN automation_retries;
ALTER TABLE tasks DROP COLUMN automation_checked_at;
ALTER TABLE tasks DROP COLUMN automation_detail;
