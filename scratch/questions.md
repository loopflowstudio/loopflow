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
- **The single-Session read drops ended agents too** (concept review). Dead
  for an ended agent comes from the row in `read_summary` for every read, and
  `SessionProcessIdentity.completed_at` goes. The kickoff plan narrowed only
  the all-read.
- **The deadline statement shares its guard with `available_sql`** rather than
  repeating three of its clauses.
- **The all-read's identities ignore `completed_at`.** A completed Session
  whose agent has not ended is asked about in both reads; the plan's wording
  ("add" two clauses) would have kept `completed_at IS NULL` in one of them.
- **`INDEXED BY agent_sessions_unended_agent` on the all-read.** It pins the
  driver the design names; drop it if the planner should be free to choose.
- **A possibly order-dependent test**, `resuming_an_ended_conversation_launches_its_provider_as_the_session`,
  failed once under threaded `cargo test` and not alone. Not investigated.
