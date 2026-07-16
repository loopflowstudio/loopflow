-- A parent process id must name a process this ledger recorded. Rows written
-- before the writer enforced that (journal::ensure_run_context) point at
-- parents that reached no store on the machine: a development build whose
-- write the production-store guard refused, or a `lf wave` listener that
-- outlived the store it minted its id against. Both exported LF_PROCESS_ID
-- and let children stamp it, because the ledger insert is best-effort and
-- swallows its failures.
--
-- Apply the writer's invariant to that history once. The pointer is the only
-- false thing here — every run, every row, and every token stays; the child
-- becomes a root of the trace it really ran under, which is what the ledger
-- can honestly say about a parent it never saw.
UPDATE run_events
SET parent_process_id = NULL
WHERE parent_process_id IS NOT NULL
  AND parent_process_id NOT IN (SELECT process_id FROM run_events);
