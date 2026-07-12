-- The 061 scaffold was applied before the production launch gate existed.
-- Coverage begins when the repaired contract is active, not when its unused
-- tables first appeared.
UPDATE trace_capture_meta SET required_after = unixepoch() WHERE id = 1;
