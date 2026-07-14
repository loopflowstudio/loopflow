-- Pin each Session's execution context, and record terminal intent durably.
--
-- lf_bin/db_path/lf_home are the binary and store a Session relaunches with.
-- They are nullable because a Session created before this migration genuinely
-- has no pinned context, and none can be invented for it: nothing recorded which
-- `lf` spawned it. An unpinned Session refuses to launch and says so, rather than
-- guessing from whichever process happens to be holding it — and that guess is
-- the exact bug these columns exist to kill.
--
-- abandon_* records that abandonment was *requested*. That is decided the moment
-- the command is queued, not when a runner finally consumes it; the gap between
-- the two is where a supervisor used to restart work someone had already ended.

ALTER TABLE task_sessions ADD COLUMN lf_bin TEXT;
ALTER TABLE task_sessions ADD COLUMN db_path TEXT;
ALTER TABLE task_sessions ADD COLUMN lf_home TEXT;
ALTER TABLE task_sessions ADD COLUMN abandon_requested_at INTEGER;
ALTER TABLE task_sessions ADD COLUMN abandon_reason TEXT;

ALTER TABLE project_sessions ADD COLUMN lf_bin TEXT;
ALTER TABLE project_sessions ADD COLUMN db_path TEXT;
ALTER TABLE project_sessions ADD COLUMN lf_home TEXT;
ALTER TABLE project_sessions ADD COLUMN abandon_requested_at INTEGER;
ALTER TABLE project_sessions ADD COLUMN abandon_reason TEXT;
