# Trace

Every run explains and replays itself, locally. Any question about what
happened, what it cost, and why is answerable on your own machine for the
life of the system — and a run can be replayed, not just read. Personal
deployments only: no remote telemetry server, ever (a bound, not a gap).

## KRs

- Runs reconstructable end-to-end: prompt, context, flow/skill shape,
  spawned work, cost, time, result.
- `run_token_usage` is fed at run completion (0 rows against 152 runs
  today; the schema and upsert path already exist) and `lf usage` shows
  real numbers; cost and per-skill dimensions get schema homes.
- Empirical stats over your own history are a usable surface (which flows
  run hot, what they cost, how they trend).
- A recorded run can be replayed against the same context for debugging
  and for evals; paved-road deviations are visible in the record.
