-- The launching process's CPU sample of its provider body, every 15 s: when it
-- last sampled and since when the body and its tools used no CPU. Updated in
-- place; store_revision_session_activity_update does not list these columns,
-- so samples never advance the `sessions` revision.
ALTER TABLE session_activity ADD COLUMN cpu_sampled_at INTEGER;
ALTER TABLE session_activity ADD COLUMN cpu_quiet_since INTEGER;
