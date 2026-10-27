# LOO-604: one lf API for Loopflow state

Status: implementing; `lf monitor work` deleted (2026-10-26). Jack Heart requested the Task;
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
  internally consistent, panes no longer share one snapshot. Done: the monitor,
  its part vocabulary and `session_watch.rs` are gone; `watch.rs` is the one
  `--watch`.
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

`--watch` (`lf/commands/watch.rs`, one implementation): a frame
`{answers, home, copy, revisions, unavailable, body}` per line, `body` = the
`--json` output, sent first and whenever its identity (reading-time fields and
`Watched::stamp`, on the result or each row, stripped) changes. Wakes on the
read's revision domains (all but usage), its clock, or — with
`Watched::following_checkouts` — a Task checkout whose Git answer changed;
settles bursts; rests `BUSY_SHARE` × last cost; retries failures with backoff.
A store that exists but cannot be opened is `unavailable`, not empty. Stdin
`{"id": N, "args": {...}}` replaces the arguments (`args` absent = confirm); the
next frame answers N. Ends on stdin close, parent exit or a failed write.
Usage is in `docs/planning.md`.

Desktop (`WorkWatches`, `swift/Loopflow/Services/WorkObservation.swift`) runs per
window: `wave show --all` (plan; Waves derived from it), `desktop plans`
(Dashboard plans), `planning status` (receipts; holds the repository's planning
sync), `session list`, `task status <id>` (overview, Graph processes, comments,
recorded work), `history --since 7d --limit 50 [filters]`, `monitor top`. Wave
detail is the selected Wave's `WaveRoadmap` from the planning watch. A scope change restarts watches whose argv/cwd changed and
replays the others' last frame answering the new id. A checkpoint confirms each
running watch (or only `replay` parts) and follows once each answered with a
reading begun after the request. Any watch ending ends the observation and the
window reopens all.

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

An orphaned reader (from an exited Loopflow Dev.app) had run for 3h37m at 99%
CPU: a descendant still held its stdin open, which was the only stop signal.

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
- `lf monitor work` deleted (`work_watch.rs`, its part/heartbeat/scope
  vocabulary, the task_history part, `work_frame.json`); its checkout follower
  moved to `watch/checkouts.rs`. New watches: `wave show`, `wave status`,
  `planning status` (with the sync lifetime; works in a non-Git Work folder),
  `history`, `desktop plans` (new command), `monitor active` (onto `watch.rs`;
  `session_watch.rs` and its `refresh`/`rescan` requests deleted).
  `TaskStatus` gains `overview`, `graph_processes`, `comments`; Desktop's Debug
  disclosure reads `execution.work` from it. Watch frames carry `home`/`copy`.
  Deleted `next_quiet_available`, `roadmap_all`, `current_waves`,
  `observeTaskDiagnostics`. Fixture: `watch_frames.json`.
- One Task row: `TaskDetailSnapshot` and Swift `WaveTaskWork` are deleted;
  `wave status` Tasks and `--done` pages are `RoadmapTask` rows with `section`.
  Their `direction` (Rust `{text}`) never reached Desktop, which decoded a
  `directive` key no reader emitted; both are deleted with the per-reading
  steers statement and `durable::render_steers`. `closed_task_page.json` was
  stale after main's Auto-Ship merge (`human`, `completed`, no `ship_control`).
- Wave detail: Desktop's `lf wave status <id>` watch is deleted (`WorkPart.wave`,
  `WorkScope.wave`, `WavePart`, `detailWaveId`). The pane reads the selected
  Wave's `WaveRoadmap` from `wave show --all`, which already carries readiness,
  metrics, Projects, unavailable Tasks and its open Task rows; the snapshot's
  `history` was never rendered and Session history has its own watch. The
  detail lost only the PR `empty` probe, which Desktop decoded but never read.
  `lf wave status [--watch]` stays an ordinary CLI read.
- Comments: Desktop's one-shot `lf task comment` read is deleted
  (`RegistryQuery.taskComments`, `WorkModel.loadComments`,
  `TaskCommentsProofTests`); the thread arrives with the `task status` watch,
  a failed reading marks it unavailable, and retry asks the watches to reread.
  Production `WorkModel` always has the watch; only tests lacked one.

## Plan per read

Scope: M machine, R repo, W wave, T task. Rows already on a `--watch` are done
unless "Work needed" says otherwise.

| Read | Desktop caller / cadence | Scope | Work needed |
|---|---|---|---|
| `wave show --all` | watch | M | Waves and Projects ride the plan |
| `session list` | watch | R | page or diff (Remaining 1) |
| `task status <id>` | watch | T | 5k statements, 20 connections (Remaining 2) |
| `history --since 7d` | watch | W/P/T | |
| `monitor top` | watch | M | shrinks with LOO-615 |
| `planning status` | watch | R | holds the repository's planning sync |
| `desktop plans` | watch | R | |
| `wave status` per Wave | `openHistoricalReference`, user action | W | check historical Projects are in the plan |
| `wave list --all --current` | before first frame; after createWave | M | delete once a checkpoint covers createWave |
| `desktop view --plan` | each planning frame while a `.view` is open | R | ad-hoc expression in `desktop plans` args |
| `wave workflow list`, `graph list` | frames whose planning moved; palette | R | catalog watch (Remaining 3) |
| `portfolio`, `machine id/list` | didBecomeActive, appear | M | machine watch (Remaining 3) |
| `graph list/show --for-task` | non-window or remote Tasks | T | remote keeps one-shot |
| `task file/diff/changes`, `task location`, `repo identity` | file events, user action | T | explicit reads; Process rows → LOO-615 |
| `usage`, `repo tokens`, `doctor`, `usage --task` | Telemetry, expand | M/R/T | explicit; transcripts → LOO-614 |

## Remaining work

1. Sessions: incremental or paged rows instead of 5.7 MB per reading; a quiet
   Session turns Waiting on the 2 s clock (`next_quiet_available` is gone).
2. `lf task status`: 5k statements / 20 connections for one Task, now with
   overview, Graph processes and comments, on a 2 s clock; reuse
   `TaskDetailFacts` and `GraphExecutionFacts`.
3. Delete remaining Desktop duplicates; catalog and machine reads
   (`wave workflow list`, `graph list`, `portfolio`, `machine id/list`).
4. Known differences from the monitor: usage alone no longer refreshes Wave
   detail token totals (next change or 300 s clock); a store that cannot derive
   Task state fails `lf task status` too, so the Task pane keeps its last
   reading marked unavailable (the monitor read only `task_overview`); a
   confirmation re-reads each watch (the monitor replayed unchanged parts).
5. Live p99 A/B with an installed build: gate/demo.

## Delete — do not maintain

- Done: `RegistryQuery.plan/waves(repoPath:)/userName/taskFiles/repositoryWaves`,
  WavesView per-Wave `lf wave status`, `lf monitor work` and `work_watch.rs`,
  `session_watch.rs`, task_history part, `work_frame.json`, `LocalWorkObservation`'s
  single reader, Desktop's `wave status` watch, `taskComments` read.
- Next: `allWaves`
  after createWave, no-stream-only reads (`roadmap`, `sessionPage`,
  `peerPlanningStatus`, `workActivity`) if the one-shot transport is not kept
  for remote Machines.

## Check results

- Earlier slices (open-Task scope, batching, `task list [query]`, `--watch` on task/session list, task status, monitor top): passed; detail in git history of this file (515ed2f3bf).
- Pass b6772c9e3d (monitor deletion): clippy clean; watch lib/integration tests, reference, Desktop suites (130) passed; Linux `planning_reconnect` e2e: CI.
- One Task row: clippy clean; `--test integration dto_fixtures` 43 passed; `test_desktop.sh --filter 'DTOFixtureTests|WaveLensTests|RegistryQueryTests|TaskHistoryFilterTests|WaveDetailReadingTests|MockWaveFixtureTests'` 103 passed; waves lib tests: gate.
- Wave detail from planning (Swift only): `test_desktop.sh --filter 'WaveDetailReadingTests|MockWaveFixtureTests|DTOFixtureTests|WorkObservationTransportTests|DesktopPerformanceTests|RegistryQueryTests|WorkModelTests|WorkSelectionTests'` 128 passed; UI capture: gate.
- Comments from the watch: `test_desktop.sh --filter 'DesktopHeadlessTests|WorkModelTests|DTOFixtureTests|WorkSelectionTests'` 100 passed.
