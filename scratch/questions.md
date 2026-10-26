# LOO-604 open questions

- Assumption (2026-10-26): on a slow store, live parts may arrive up to
  `BUSY_SHARE` × projection time late (~8 s for planning on Jack Heart's store)
  in exchange for bounded CPU. User actions confirm via checkpoint, which never
  waits. Reversible: raise or remove `BUSY_SHARE` in `work_watch.rs`.
- Assumption: Waves outside `planning.roadmap` (abandoned/retired or
  authored-only) show objective-only rows in WavesView, without Projects.
  Previously each got a `wave status` read.
- Not done: pid 5210 (orphaned reader, 99% CPU since ~08:30) is still running on
  Jack Heart's Mac; it was left alone. `kill 5210` stops it; the fix prevents new ones.
- Assumption (2026-10-26): a done/abandoned Task whose execution is still
  unresolved (live Graph, unsettled checkout) leaves planning with the rest of
  the closed Tasks; its row, with `unresolved_execution`, is in
  `lf task list --done`. Keeping it in planning would need its Git probes for
  every closed Task. Reversible: add those Tasks to the open predicate.
- Assumption: `--done` pages done and abandoned Tasks together (both left the
  plan), ordered by abandonment, else completion, else last update.
- Note (2026-10-26): LOO-594's steer says "keep the closed row type as
  RoadmapTask", but closed pages return `wave status` rows (Swift
  `WaveTaskWork`) so Desktop can merge them with the Wave view's open rows.
  They switch to `RoadmapTask` with Wave detail becoming Task rows (Remaining 2).
- Assumption: a graph's inventory entry in Task reads names the Task being read
  and its Wave, not `task_of_process`'s longest-worktree owner; they differ only
  for a Graph run in a nested Task checkout.
