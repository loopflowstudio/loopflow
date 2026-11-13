- LOO-866: `resolve_task_id_in` (children.rs) scans `tasks` on every Task
  drill (short-id prefix `substr(t.id,6,…)` and the `task_creation_intents`
  EXISTS defeat MULTI-INDEX OR). Constant per drill, ~10k Tasks; left out of
  this Task. `session_history_reads_plan_by_window` checks only the history
  statement of the drill. Candidate follow-up Task.
