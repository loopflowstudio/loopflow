# LOO-643 open questions and assumptions

- Assumption (kickoff, 2026-10-27): the repair lives in `lf monitor prune`, which
  already reaps AgentProcesses and reports per-record actions. It does not get a new
  `lf self doctor --repair` flag. That keeps doctor read-only and reuses an existing
  maintenance command. Reversible: the pass is one function either command could call.
- Resolved (implement, 2026-10-27): released ancestry uses `attachment_token = NULL`.
  `lf clone`/`store/copy` already null every token, so readers (`session_activity`
  joins, steers, status projections) treat NULL as "no attachment".
- Decision (implement, 2026-10-27): attachment APIs keep `&LfProcessId` rather than
  `&LfProcess`. Every production caller passes `journal::current_lf_process_id()`, so
  `attach_in` writes `journal::current_start_record(process)` when the attacher is the
  current Process. This avoids rewriting ~150 test call sites; tests that attach an
  unrecorded id now record it (the deleted COALESCE no longer roots it).
- Not verified: the 05:26 UTC process behind `4aaad068` is unrecorded and not
  identifiable from the store or logs. The design does not depend on identifying it.
