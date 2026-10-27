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
- Assumption (2026-10-26, frame per Jack Heart 2026-10-27): a `--watch` frame
  wraps the `--json` result as `{answers, unavailable, body}`; stdin requests
  are `{"id", "args"}` with flag names. Pinned by `watch_frames.json`.
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
- Assumption (2026-10-26): Wave detail for a Wave outside the current plan
  (abandoned or retired) shows its cached plan with "not in the current plan"
  instead of a live `lf wave status` reading. Reversible: one-shot
  `RegistryQuery.status` for those Waves on selection.
- Assumption (2026-10-27): Session rows carry liveness through their existing
  `state` and `agent_process_id`, not a copied list of observed provider
  processes. Sampling OS processes on every `session list` reading would add
  to Remaining 1's cost for detail `lf ps` already gives. The per-process
  activity (`waiting`/`working`) is no longer per Session. Reversible: add
  `processes` to the row from `lf ps`'s sample.
- Assumption (2026-10-27): `TaskRow.wave`/`project` are names (what `--wave`
  and `--project` accept), `reason` is the next-move reason, `pr` appears once
  published on GitHub, and `next` is `lf task resume` (deferred), `lf task
  complete` (merged PR), else `lf task run` when runnable; `null` while moving,
  shipping or closed. Reversible in `task_row.rs`.
- Assumption (2026-10-27): `lf task status --json` drops worktree, branch,
  ownership, planning evidence and project records; they stay in `lf desktop
  task` (tests needing them read it). `lf task location` gives the checkout.
  An unregistered (Linear-only) Task is an error on `lf task status --json`.
- Assumption (2026-10-27): Desktop's reads are `lf desktop plan|done|wave|task`
  rather than one combined Desktop read; each is today's rich shape verbatim.
- Assumption (2026-10-27): the store/build a Desktop observation reads comes
  from one `lf --version` per observation (`copy:` line), not per frame;
  Machine identity stays with `lf machine id`.
- Assumption (2026-10-27): `SessionRow.task` is the Task id (from the Session's
  recorded Work), not its identifier, which would need another lookup per
  reading; `wave` is the Wave name. Reversible in `SessionRow::of`.
- Assumption (2026-10-27): Desktop's Git planning disclosure no longer lists
  journal entries; it points to `lf planning journal`. Reversible: a one-shot
  journal read when the disclosure opens.
- Assumption (2026-10-27): with the Wave selected, no watch follows a closed
  Task; an edit to it reaches Desktop when the Task is shown again
  (`taskLifecycleUpdatesOpenSidebar` asserts after `goBack`).
- Assumption (2026-10-27): `lf task status` reads each of the Task's Sessions
  by id (`human_session::listed`), not through `lf session list --task`, which
  scans every Session (Remaining 1's cost). A Session whose row is gone is
  omitted. Its `available` uses the same per-id judgment `lf session show` uses.
