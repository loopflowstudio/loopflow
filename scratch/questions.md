# LOO-604 open questions

- Assumption (2026-10-26): on a slow store, live parts may arrive up to
  `BUSY_SHARE` × projection time late (~8 s for planning on Jack Heart's store)
  in exchange for bounded CPU. User actions confirm via checkpoint, which never
  waits. Reversible: raise or remove `BUSY_SHARE` in `watch.rs`.
- Assumption: Waves outside `planning.roadmap` (abandoned/retired or
  authored-only) show objective-only rows in WavesView, without Projects.
  Previously each got a `wave status` read.
- Assumption (2026-10-26): a done/abandoned Task whose execution is still
  unresolved (live Graph, unsettled checkout) leaves planning with the rest of
  the closed Tasks; its row, with `unresolved_execution`, is in
  `lf task list --done`. Keeping it in planning would need its Git probes for
  every closed Task. Reversible: add those Tasks to the open predicate.
- Assumption: `--done` pages done and abandoned Tasks together (both left the
  plan), ordered by abandonment, else completion, else last update.
- Assumption (2026-10-26): Task rows drop `direction`. Desktop never read it
  (it decoded a different key); steers stay visible through `lf task status`
  comments. Reversible: add a field to `RoadmapTask` that Desktop renders.
- Assumption: a graph's inventory entry in Task reads names the Task being read
  and its Wave, not `task_of_process`'s longest-worktree owner; they differ only
  for a Graph run in a nested Task checkout.
- Assumption (2026-10-26): a `--watch` frame wraps the `--json` result as
  `{answers, home, copy, revisions, unavailable, body}`; stdin requests are
  `{"id", "args"}` with flag names. Pinned by `watch_frames.json`.
- Assumption: `lf task status --watch` exchanges planning only for its first
  reading, then rereads the store; Desktop's planning exchange is held by
  `lf planning status --watch` (one per window with a repository).
- Assumption (2026-10-26): Desktop's planning watch is `lf wave show --all`
  (Waves and Projects come with the plan), not `lf wave list --watch`, which
  would carry no Projects and read the plan twice.
- Assumption: the Task pane's shape comes from `lf task status` (now with
  `overview`, `graph_processes`, `comments`), rather than Desktop assembling it
  from separate reads. Every `lf task status --json` caller pays for them.
- Assumption: Dashboard selections live in a new `lf desktop plans --json
  [--watch]` (the repository's default and saved plans).
