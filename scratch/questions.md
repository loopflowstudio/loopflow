# Open choices (LOO-865)

- Graphs that ran in a checkout before a Task adopted it (`lf task create --worktree`, `lf session bind`) stop counting as that Task's Graphs once the association is recorded at launch. The live store has one such Graph. Took the simpler option: no backfill on adoption. If adoption should claim earlier Graphs, the adopting write would need its own one-time UPDATE, which the append-only Graph record currently forbids.
