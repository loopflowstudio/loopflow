# Open choices, LOO-979

Made without anyone to ask; each is reversible.

- **No index on `session_activity(observed_at)`.** The Task brief suggests one
  "if none serves it". The read is driven from `agent_sessions_unended_agent`
  instead, so no index is written on every activity reading.
- **Identities and Graph runs are narrowed too**, not only deadlines: they
  were 60% of the read's time. A completed Session with an ended agent then
  reads Dead in an all-Sessions summary where it read Unknown.
- **`graph_processes` stays out of `GROWING`.** Worth its own Task if Graph
  statements should be plan-checked: it needs `graph_processes_unended` and
  `graph_process_steps_*` measured and every traced Graph statement audited.
- **`pin` still reruns the inputs.** Comparing `store_revisions` would need
  `store_revision_agent_sessions_update` to fire on every identity write.
