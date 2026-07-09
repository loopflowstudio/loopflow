# Trace

Every run explains and replays itself, locally. Any question about what
happened, what it cost, and why is answerable on your own machine for the
life of the system — and a run can be replayed, not just read. Personal
deployments only: no remote telemetry server, ever (a bound, not a gap).

## KRs

- A month of runs is 100% reconstructable — prompt, context, flow/skill
  shape, spawned work, cost, time, result — verified by random spot-audits
  that succeed N/N.
- `run_token_usage` is fed continuously from the day it's wired: zero
  gap-days in a month (0 rows against 152 runs today; the schema and
  upsert path already exist), and cost + per-skill dimensions get homes.
- Any run from the last month can be replayed against its recorded context
  for debugging and for evals.
- The stats surface answers the standing questions — what runs hot, what
  it costs, how it trends — in one query, on real history.
