-- Capture-scope auditing changed after 066 while development still ran through
-- the installed pre-capture binary. Activate the final contract after that
-- mixed-version interval so known development gaps do not stay permanently red.
UPDATE trace_capture_meta SET required_after = unixepoch() WHERE id = 1;
