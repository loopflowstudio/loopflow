-- Development commands after 065 still ran through the installed pre-capture
-- binary. Activate coverage from the finalized launch gate, not that mixed
-- binary interval.
UPDATE trace_capture_meta SET required_after = unixepoch() WHERE id = 1;
