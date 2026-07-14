-- 064 activated coverage before one-shot launches persisted provider receipts.
-- Start the contract after the complete launch gate is installed.
UPDATE trace_capture_meta SET required_after = unixepoch() WHERE id = 1;
