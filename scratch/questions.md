# LOO-859 open questions

- Skipped-sync line kept as docs/work.md shows it:
  `billing: no Linear Initiative; showing local planning (lf wave plan connect billing)`.
- `wave.sync --json` carries no "skipped" field. Assumed: not needed. Desktop's
  clock sees success and keeps its 5-minute cadence; each call is now a local
  read with no Linear request or write lock. docs/work.md no longer claims
  Desktop stops refreshing.
- Stripping `acquisition.observed_at` / `project_readiness.observed_at` from
  frame identity means Desktop sees those stamps only with the next real change.
  Assumed acceptable, as for `condition.observed_at`.
