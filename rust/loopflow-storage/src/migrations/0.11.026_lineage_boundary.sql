-- A parent process id must name a process this ledger recorded. Rows written
-- before the writer enforced that (journal::ensure_run_context) point at
-- parents that reached no store: the ledger insert is best-effort, and a
-- process exported its identity to children whether or not its own row landed.
--
-- Apply the writer's invariant to that history once. The pointer is the only
-- false thing here — every run, every row, and every token stays; the child
-- becomes a root of the trace it really ran under.
UPDATE run_events
SET parent_process_id = NULL
WHERE parent_process_id IS NOT NULL
  AND parent_process_id NOT IN (SELECT process_id FROM run_events);
