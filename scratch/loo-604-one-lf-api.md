# LOO-604: one lf API for Loopflow state

Status: draft plan + first slice (2026-10-26). Jack Heart requested the Task;
his LOO-602 steer (same day) sets the data the API reads: a facts-only store
(LOO-611 Session facts + one usage record per turn, LOO-612 ProviderFact,
LOO-615 Process rows deleted at exit unless referenced). Transcript reads
(history show, search, retention) belong to LOO-614's per-harness readers, not
to store queries.

## Decisions

- Jack Heart, 2026-10-26: completed Tasks leave the live planning part for a
  paged history read. Planning carries incomplete Tasks only, so the
  per-reading Task set no longer grows with history (Jack's store: 229 of 771).
- Jack Heart, 2026-10-26: "we should focus on incomplete tasks and make
  searching among them fast and easy". Search is `lf task list [query]`
  (identifier/title; `--wave`, `--project`, `--state`), not a separate
  `lf task search`.
- Jack Heart, 2026-10-26 (concept review, option B): the live API is `--watch`
  on the ordinary reads; the command a client types one-shot is the one it
  streams. One open-Task row, `RoadmapTask`, shared by `lf task list --json`
  and planning; closed pages use it too. Wave detail becomes Task rows
  filtered by `--wave` plus the Wave's own fields.
- Jack Heart, 2026-10-26: delete `lf monitor work`. Desktop runs one process
  per watch (`lf task list --watch`, `lf session list --watch`,
  `lf task status <id> --watch`, the live-process read); each watch is
  internally consistent, panes no longer share one snapshot. The watch
  machinery moves out of `work_watch.rs` into the shared reads; the monitor,
  its part vocabulary and its Desktop transport go in the change that moves
  Desktop over.
- Jack Heart, 2026-10-26: `lf history` is Session history: Session records
  (turns, usage, Processes they ran, `show`, replay) tagged with Task/Wave/
  Project; `--task/--wave/--project` filter by tag. Work changes no Session
  made (sync completion/moves, Workflow moves) live on the Task (`lf task
  status`, comments, Workflow history), not in `lf history`.
- Monitor parts, from those decisions: work_activity → `lf history [--wave|
  --project|--task] --since --watch`; task_history → deleted (`lf task status`
  already returns `task_work`); peer_planning → per-Task `task.sync` on rows,
  repository sync stays `lf repo planning status` read on open.
- LOO-594 alignment (Jack Heart, 2026-10-26): the standup Done column is one
  read, `lf task list --done --wave W --since <7d|RFC3339>`, bounded and
  indexed. Both branches touch TaskHistoryFilter(.swift/Tests) and
  DTOFixtureTests: keep one filter; whichever lands second rebases.

## Direction

Order: Graph execution batched (done), `lf task list [query]` (done),
`--watch` (done for task list, session list, task status, monitor top), then
Desktop moves over and `lf monitor work` is deleted.

`--watch` (`lf/commands/watch.rs`, one implementation): a frame
`{answers, revisions, unavailable, body}` per line, `body` = the `--json`
output, sent first and whenever its identity (reading-time fields stripped)
changes (`Watched::stamp` names a top-level field that restates the reading
time, such as the activity snapshot's `observed_at`). Wakes on the read's revision domains (all but usage) or its clock
(task list 300 s, session list, task status and monitor top 2 s); settles
bursts; rests
`BUSY_SHARE` × last cost; retries failures with backoff. Stdin
`{"id": N, "args": {...}}` replaces the arguments (`args` absent = confirm);
the next frame answers N even if unchanged — this is the monitor's scope +
checkpoint in one. Ends on stdin close, parent exit or a failed write.
`task status --watch` runs the one-shot status (with its planning exchange)
once, then re-reads `read_task_status` read-only. The monitor imports the
shared constants, `retry_due` and `strip_reading_time` until it is deleted.
Usage is in `docs/planning.md`.

## Measured on Jack's store (10.3 GB, 762 Tasks, 6,248 Sessions, 944k Processes)

Installed lf 0.14.15, one reader with a repo scope, idle window:

| Part | Per reading | Readings / 60 s | Notes |
|---|---|---|---|
| planning | avg 1.6 s, max 3.6 s; ~34k statements; 5.8 MB | 20–24 (8 sent) | per-Task N+1 over all 762 Tasks; every statement < 1 ms |
| sessions | avg 0.36 s; 5.7 MB | 20–28 | every Session of the repo, unpaged; 2 s blind clock |
| activity | avg 0.19 s; ~4k statements | 18–25 | 4,206 unfinished process rows |
| wave | ~1–2 s; 3.6 MB | on change | |
| work_activity | ~1–1.5 s | 2 s clock | |
| task | 0.2–0.7 s | 2 s clock | `lf task status LOO-604`: 5.8 s, 5,019 statements, 20 connections |

One idle reader used ≈48 s CPU per 60 s. Revisions advance ~0.5/s each for
planning and sessions, so the reader is correctly woken; the projections are too
expensive. Individual statements are fast, so indexes are not the main gap:
statement counts are. The one known scan (agent_sessions by execution owner)
is indexed by #1692, merged into this branch.

An orphaned reader (pid 5210, from an exited Loopflow Dev.app) had run for 3h37m
at 99% CPU: a descendant still held its stdin open, which was the only stop
signal.

## This change

- Reader exits with its parent (checked each heartbeat), not only on stdin
  close. Verified live: 0.14.15 outlives a killed parent; the branch exits in 1.5 s.
- A part's change- or clock-driven reading waits `BUSY_SHARE` (5) × its last
  reading's duration. Scope changes, refresh and checkpoint read at once.
- Incomplete-Task scope: planning (`wave show`), `wave status` and `lf task list`
  read open Tasks only (`list_open_tasks`, `task_open_sql`); an exact
  `--task` lookup still finds a closed one. `lf task list --done [--wave]
  [--after ID] [--limit N] --json` pages closed (done or abandoned) Tasks, most
  recently closed first, as `wave status` Task rows (`closed_task_page.json`).
  Desktop's Wave Done view pages them in while "completed" is on (back to the
  range start; "Older completed Tasks" for more). `active_tasks` is one count.
- Task details are read once per reading over the Tasks in scope
  (`TaskDetailFacts`, `store/sqlite/task_details.rs`): state, active and parent
  PR, completion request, issue URL, owner, acquisition, latest event, steers,
  Workflow name, planning sync, repository paths; list reads resolve every
  identifier in one statement (`TaskIdentifiers`). Per-Task reads now delegate
  to the same batch SQL (acquisition, planning sync, ownership log). Measured:
  5 Tasks 164 → 42 statements, 10 Tasks 309 → 42.
- Graph execution is read once per reading over the scope's latest Graphs
  (`GraphExecutionFacts`: steps in one statement, captured inputs through one
  recursive CTE, receipts of unfinished Processes); the entry takes the Task's
  own Wave instead of re-deriving the Task. `task_checkouts` resolves
  identifiers with `TaskIdentifiers`. Planning statements stay flat as Tasks
  with checkouts and Graphs are added.
- `lf task list [query] [--wave] [--project] [--state]`: open Tasks from one
  store filter (`open_tasks_matching`) instead of a whole roadmap, as
  `RoadmapTask` rows in plan order.
- `lf task list --done --since <7d|24h|30m|RFC3339>` bounds closed pages by
  closing time (LOO-594's standup Done column is one read). Not indexed: the
  filter scans the repository's closed Tasks (~540 on Jack's store), which
  needs no migration.
- `lf monitor top --json --watch`: the live-process read through the same
  watch.
- `lf history` is Session records only: input captured, recorded completion,
  provider turns, tagged with Task/Wave. `WorkCreated`, `Pr*` and `SteerIssued`
  are deleted with their per-Task PR scan, `steers_since`, `SteerComment` and
  the Work creation timestamps that fed them; Desktop's activity view drops
  their icons and the PR link.
- Desktop: WavesView reads `planning.roadmap.waves[].projects` instead of one
  `lf wave status` per Wave. Deleted dead `RegistryQuery` reads.

## Plan per read

Scope: M machine, R repo, W wave, T task. "Stream" = already a part.

| Read | Answers | Desktop caller / cadence | Scope | Place in the API | Work needed |
|---|---|---|---|---|---|
| planning (`wave show --all`, `wave list --all --current`) | roadmap, current Waves, Dashboard plans | stream | M | `lf task list --watch` + Wave fields | open Tasks only, batched details and Graphs (done); `--watch` |
| sessions (`session list --page`) | Session rows | stream (headless: false) | R | part | page or diff; drop 2 s blind clock once liveness is a signal |
| task (`task status`) | TaskOverview, Graph processes, comments | stream | T | `lf task status <id> --watch` | 5k statements, 20 connections (Remaining 5) |
| task_history | TaskWork diagnostics | stream, on request | T | deleted | duplicate of `task status`'s `task_work` |
| wave (`wave status`) | Wave detail | stream | W | `lf task list --wave W --watch` + Wave fields | details batched (done); rows become `RoadmapTask` |
| work_activity (`history`) | 7-day activity | stream | W/P/T | `lf history --since --watch` | Session records only (done); `--watch` |
| activity (`monitor top --json`) | live call trees | stream | M | `lf monitor top --json --watch` (done) | shrinks with LOO-615 |
| peer_planning (`planning status`) | sync receipts | stream | R | per-Task `task.sync` on rows; repo read on open | |
| `wave status` ×N | Wave Projects | WavesView, every planning frame | W | deleted (this change) | |
| `wave status` per Wave | historical Project lookup | `openHistoricalReference`, user action | W | stream `wave` or a Project lookup | check historical Projects are in roadmap |
| `task comment` | comments | TaskCommentsView on appear | T | stream `task.comments` | delete |
| `wave list --all --current` | Waves | before first frame; after createWave | M | stream `planning.waves` | delete once a checkpoint covers createWave |
| `desktop view --plan` | ad-hoc view plan | every planning frame while a `.view` is open | R | `planning.selections` for ad-hoc expressions | add expression to scope |
| `wave workflow list` | Workflow catalog | every frame whose planning revision moved | R | new part (catalog) | |
| `graph list` | Graph catalog | palette, didBecomeActive | R | new part (catalog) | |
| `portfolio`, `machine id`, `machine list` | Machines, repositories | didBecomeActive, appear | M | new part (machine) | |
| `graph list/show --for-task` | Graph logs | non-window or remote Tasks | T | stream `task` for the window Task | remote keeps one-shot |
| `task file/diff/changes`, `task location`, `repo identity` | checkout content | file events, user action | T | explicit reads (content, not state) | 379k + 165k Process rows: LOO-615 |
| `usage`, `repo tokens`, `doctor` | telemetry | Telemetry view on open | M/R | explicit reads | |
| `usage --task [--context]` | per-Task usage | expand | T | explicit; transcript parts → LOO-614 | |

## Remaining work

1. Gaps between the watches and what Desktop reads from the monitor, found
   while sizing the move:
   - Machine identity: monitor frames carry `home`/`copy` and Desktop drops
     Machine content when they change; watch frames carry neither.
   - Planning: the part is `{selections, roadmap, waves}`. `lf task list`
     returns `RoadmapTask` rows only; Waves/Projects need their own watched
     read (`lf wave list --json --watch`), and Dashboard `selections` need a
     home (ad-hoc view plans, `lf desktop view --plan`).
   - Task pane: the part is `{work: TaskOverview, graph_processes, comments}`;
     `lf task status --json` is `TaskStatus`. Either status carries those or
     Desktop reads them from status's shape.
   - Checkpoint: `confirmWorkUpdate` waits for one answered checkpoint; with
     several watches it waits for each affected watch's answer to a request.
   - `lf history --watch` does not exist yet.
   `lf task list --watch` does not follow checkout Git changes (the monitor's
   `checkouts` watch); it rereads on its 300 s clock. Session list uses the
   2 s clock, not `next_quiet_available`.
2. Move Desktop onto those watches; Wave detail becomes `RoadmapTask` rows
   filtered by `--wave` (closed pages switch from `wave status` rows to
   `RoadmapTask` with it); delete `lf monitor work`, its parts and transport,
   and the task_history part. work_activity → `lf history --since --watch`.
3. Sessions: incremental or paged rows instead of 5.7 MB per reading.
4. `lf task status`: 5k statements / 20 connections for one Task; reuse
   `TaskDetailFacts` and `GraphExecutionFacts` for its Task. Needed before the
   Task pane watches it on a 2 s clock.
5. Delete remaining Desktop duplicates; catalog and machine reads.

## Delete — do not maintain

- Done: `RegistryQuery.plan/waves(repoPath:)/userName/taskFiles/repositoryWaves`,
  WavesView per-Wave `lf wave status`.
- Next: `lf monitor work` (`work_watch.rs` parts vocabulary, Desktop
  transport), task_history part, `taskComments` read, `allWaves` after createWave, no-stream-only reads
  (`roadmap`, `sessionPage`, `peerPlanningStatus`, `workActivity`) if the
  one-shot transport is not kept for remote Machines.

## Check results

- `cargo test -p loopflow --lib work_watch`: 29 passed (first slice).
- `scripts/test_desktop.sh --filter 'RegistryQueryTests|DTOFixtureTests'`: 63 passed (first slice).
- `cargo test --test integration -- planning_statements closed_task_pages lifecycle_world`: 3 passed (open-Task slice; statements flat 2 → 6 Tasks).
- `cargo test --lib -- waves task_ownership planning_sync children`: 53 passed; `chapter_deletion_stays_absent…` passes once its surviving Task is started rather than completed.
- `scripts/test_desktop.sh --filter 'TaskHistoryFilterTests|DTOFixtureTests|RegistryQueryTests'`: 72 passed (Done view paging).
- `docs/lf-reference.md` regenerated for `task list --done/--after/--limit`; `reference_matches_the_compiled_cli` passes.
- `cargo test --test integration -- task_list_query planning_statements lifecycle_world`: passed (Graph batching, `task list [query]`, `--done --since`); `--test two_people`: passed.
- `cargo test --lib -- local_planning wave_resolution_matrix task_execution graph_inventory task_work`: 58 passed, 1 failed — `store::tests::separate_task_worktree_tracks_and_collapses_its_parent_pr` needs a global Git `user.email` the test HOME lacks; untouched code, left to CI (which sets one).
- `cargo clippy --all-targets -- -D warnings`: clean; `docs/lf-reference.md` regenerated.
- Compress (one `ids_json` for batch-read ID arrays, one `repository_scope` for `lf task list` reads): clippy clean; `--test integration -- task_list_query planning_statements closed_task_pages lifecycle_world` 4 passed; `--lib -- planning_sync task_ownership graph_inventory task_work children` 38 passed, same 1 global-`user.email` failure (CI).
- `--watch` (task list, session list, task status), clippy clean: `--test integration -- task_list_watch session_list_and_task_status_watch task_list_query` 3 passed; `reference_matches_the_compiled_cli` regenerated.
- `--lib -- work_watch`: 29 passed, 2 failed, both monitor tests that predate this pass and still expect the old planning shape (`planning_failure_keeps_the_stored_workflow_readable` expects a hard planning error, now `tasks: Unavailable`; `offline_cli_completion_and_reopening_reach_desktop_without_refresh` expects a completed Task in planning). They go with the monitor (Remaining 2); keep their behaviour (a failed planning read leaves the Task read available; completion reaches the client) as `--watch` tests when Desktop moves over.
- `monitor top --watch` and Session-only `lf history`: clippy clean;
  `--test integration -- session_list_task_status_and_top_watch` and
  `reference_matches_the_compiled_cli` passed; `--test session_lifecycle_tests
  -- headless_history_is_discoverable…` passed; `--lib -- activity
  work_catalog steer work_identities` passed except Pi-not-installed
  (environment) and the two monitor tests above; Swift `DTOFixtureTests` 48
  passed.
- Live p99 A/B with an installed build: deferred to gate/demo.
