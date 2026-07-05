-- The runtime-ontology collapse renamed WaveRun -> Run and edited the
-- terminal_sessions CREATE in place (wave_run_id -> run_id) without shipping
-- a migration, so dbs that recorded the old migrations kept the old column
-- and every current query breaks on them. Rename to converge. Fresh dbs
-- already have run_id; the runner tolerates the "no such column" failure for
-- this migration and records it as applied (see is_tolerated_migration_error).
ALTER TABLE terminal_sessions RENAME COLUMN wave_run_id TO run_id;
