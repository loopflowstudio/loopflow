# Trace

Every run explains and replays itself, locally. Any question about what
happened, what it cost, and why is answerable on your own machine for the
life of the system — and a run can be replayed, not just read. Personal
deployments only: no remote telemetry server, ever (a bound, not a gap).

## KRs

- A month of runs is 100% reconstructable — prompt, context, flow/skill
  shape, spawned work, cost, time, result — verified by 20/20 random
  spot-audits against the long-lived ledger.
- The trace readers survive the ledger they actually read: a month in which
  every `lf runs`, `lf trace`, and `lf usage` invocation on a real, migrated
  ledger succeeds, and none fails on a schema the migrations never produce.
- Token and cost evidence has one home and no gap-days: for a month, every
  agent-bearing run lands in the ledger carrying provider, tokens, and cost,
  and no run's spend is attributed to a nested `lf` sharing its `run_id`.
- Any run from the last month replays against its recorded context, for
  debugging and for evals: 10/10 sampled runs replay unattended.
- The stats surface answers the standing questions — what runs hot, what
  it costs, how it trends — in one query, on a month of real history.
