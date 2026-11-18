# LOO-979 verify (2026-11-18)

Outcome: verified, with two follow-ups filed.

PR #1977 merged 2026-11-18T21:14:02Z as a0548bfe28. `origin/main^{tree}` equals
this checkout's `HEAD^{tree}` (c7e8449f), so the checks below ran main's code.

Post-merge CI for a0548bfe28: `CI` (push) success, run 44399318140; `macOS on
main` (push) success, run 44399318090; `Packages (main)` success; `CI`
(merge_group) success, run 44397488775.

`EXPLAIN QUERY PLAN` with sqlite3 on a read-only backup of the Machine's store
(20,803 `session_activity` rows, 22,719 `agent_sessions`, 38 unended agents):

- deadlines, all Sessions: `SEARCH s USING INDEX agent_sessions_unended_agent (agent_process_id>?)`,
  `SEARCH act USING INDEX sqlite_autoindex_session_activity_1 (session_id=?)`
- deadlines, one Session: `SEARCH s USING INDEX sqlite_autoindex_agent_sessions_1 (id=?)`,
  `SEARCH act USING INDEX sqlite_autoindex_session_activity_1 (session_id=?)`
- identities: the same two drivers, no scan
- statement before #1977: `SCAN session_activity`
- sqlite3 `.timer`, deadline statement: old 0.003 s real (user 1.0–1.2 ms); new 0.000 s (user 51–92 µs)

Tests, `LF_*` unset, stdin closed:

- `cargo test -p loopflow-execution --lib session_projection::` — 5 passed,
  including `quiet_deadlines_come_only_from_agents_that_have_not_ended` and the plan test
- `cargo test -p loopflow-storage --lib query_plans::tests::` — 13 passed

Not checked: a `sample` of `lf server run` under about 40 writing agents. The
installed `lf` is 0.16.0+7bb879aff, one commit before the merge, so the live
server does not run this code until the next `lf install`. The SQL timing
stands in.

The plan check lives in `loopflow-execution`'s `session_projection.rs`, not in
`store::query_plans::tests::` as the brief named; `session_activity` is in
`GROWING`, so a scan fails either place.

Follow-ups filed (unstarted):

- LOO-981: an `lf` read on a copy of a Machine's store acts on the real checkouts it names
- LOO-982: `resuming_an_ended_conversation_launches_its_provider_as_the_session` failed once in a threaded run

Left in the PR body, unfiled: the third copy of the 120 s literal in the
`store_revision_session_activity_update` trigger, `graph_processes` outside the
plan check, `INDEXED BY` versus planner choice, the redundant filter in
`selections.rs`.
