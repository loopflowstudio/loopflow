-- A person's `lf ship --stop` stands until someone runs `lf ship` again: a run
-- reaching `end` approves Auto-Ship only while no stop stands. The Processes
-- name who approved and who stopped; their rows may be pruned, so no key.
ALTER TABLE tasks ADD COLUMN ship_approved_lf_process_id TEXT;
ALTER TABLE tasks ADD COLUMN ship_stopped_at INTEGER;
ALTER TABLE tasks ADD COLUMN ship_stopped_lf_process_id TEXT;
