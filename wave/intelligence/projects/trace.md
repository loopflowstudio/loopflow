# Trace

Every run explains and replays itself, locally. Any question about what
happened, what it cost, and why is answerable on your own machine for the
life of the system — and a run can be replayed, not just read. Personal
deployments only: no remote telemetry server, ever (a bound, not a gap).

## KRs

- A month of runs is 100% reconstructable — prompt, context, flow/skill
  shape, spawned work, cost, time, result — verified by random spot-audits
  that succeed N/N.
- The trace readers are exercised against a real, long-lived ledger, not
  only a fresh one: a month passes in which no `lf runs` / `lf trace`
  invocation fails on a schema the migrations never produce.
- Token and cost evidence has exactly one home, and every agent-bearing run
  lands in it: no second usage store, no run whose tokens are attributed to
  a nested child `lf` that shares its `run_id`.
- Any run from the last month can be replayed against its recorded context
  for debugging and for evals.
- The stats surface answers the standing questions — what runs hot, what
  it costs, how it trends — in one query, on real history.
