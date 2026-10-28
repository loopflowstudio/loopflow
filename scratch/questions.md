# LOO-625 merge with main (#1733 Workflow → Graph)

- #1733's `task_graphs` draft renamed `waves.workflow` to `graph`; this branch's
  `wave_setup` draft drops that column. Resolved by removing the rename from
  `task_graphs.sql` (both unreleased) so `wave_setup` drops only the released
  `workflow` column, rather than adding a drop of a column another draft created.
- Followed #1733's naming: Wave `config.yaml` key `graph:` (`workflow:` still read
  as the LOO-637 alias), `lf set graph`, `WaveSetup.graph`, `graph_source` values
  from SetupSource (`personal`, `repo`, `default`, ...). `lf wave graph set` is
  removed in favor of `lf set graph`; `lf wave graph show` stays.
- Installed lf 0.14.19 `lf sync` fails on a clean tree (`git add` of ignored
  `.lf/tmp` exclude pathspec); merged origin/main with git directly.
