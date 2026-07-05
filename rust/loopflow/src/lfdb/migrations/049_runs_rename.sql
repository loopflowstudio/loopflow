-- Drift #2 from the runtime-ontology collapse (same disease as 048): the
-- wave_runs table and the wave_run_id columns on agents/fork_runs were
-- renamed by editing historical CREATE migrations in place, stranding every
-- db that had already recorded them. Column sets are identical (verified
-- against a live 046-era db) — pure renames. Fresh dbs fail on the first
-- statement ("no such table: wave_runs"), which the runner tolerates for
-- this version and records as applied; partially-renamed dbs cannot exist
-- (all three renames shipped in one commit).
ALTER TABLE wave_runs RENAME TO runs;
ALTER TABLE agents RENAME COLUMN wave_run_id TO run_id;
ALTER TABLE fork_runs RENAME COLUMN wave_run_id TO run_id;
