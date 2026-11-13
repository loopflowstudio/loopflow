# Open choices (LOO-865)

- Graphs that ran in a checkout before a Task adopted it (`lf task create --worktree`, `lf session bind`) stop counting as that Task's Graphs once the association is recorded at launch. The live store has one such Graph. Took the simpler option: no backfill on adoption. If adoption should claim earlier Graphs, the adopting write would need its own one-time UPDATE, which the append-only Graph record currently forbids.
  The sharpest case is a Graph still running when its checkout is adopted: Task status reads "No Task Graph has run", and `lf task stop`/session moves (`stop_graphs`, `task_graphs`) don't reach it. `live_execution` still reports it as an open Process by cwd, and landing still keeps the checkout (`graphs_at`). Before this change the Task claimed it.
