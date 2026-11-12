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
- Project readiness is Unavailable when `projects.pm_snapshot_synced_at IS NOT
  pm_projects.observed_at`. Restamps of the pair don't advance planning, so a
  write that moved one without the other would flip readiness unseen. Every
  `put_project` caller projects the accepted Project in the same transaction,
  so the pair moves together; assumed sufficient rather than making the
  triggers compare across tables.
