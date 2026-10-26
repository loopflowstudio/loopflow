# LOO-604: one lf API for Loopflow state

Status: draft plan + first slice (2026-10-26). Jack Heart requested the Task;
his LOO-602 steer (same day) sets the data the API reads: a facts-only store
(LOO-611 Session facts + one usage record per turn, LOO-612 ProviderFact,
LOO-615 Process rows deleted at exit unless referenced). Transcript reads
(history show, search, retention) belong to LOO-614's per-harness readers, not
to store queries.

## Decisions

- Jack Heart, 2026-10-26: completed Tasks leave the live planning part for a
  paged history read. The planning stream carries incomplete Tasks only, so the
  per-reading Task set no longer grows with history (Jack's store: 229 of 771).
- Jack Heart, 2026-10-26: "we should focus on incomplete tasks and make
  searching among them fast and easy". The API centers on incomplete Tasks;
  searching among them by title, identifier, Wave, Project and state is a
  first-class read, scoped and indexed for the p99 goal. Completed Tasks stay
  behind the paged history read.

## Direction

`lf monitor work --watch --json` is the API. A client writes scope requests
(NDJSON on stdin) and reads parts (NDJSON on stdout): `planning`,
`peer_planning`, `sessions`, `task`, `task_history`, `wave`, `work_activity`,
`activity`, plus `heartbeat` and `checkpoint`. Every body is the wire type the
matching one-shot `--json` read prints, so a herdr/tmux client can start with
one-shot reads and move to the stream unchanged. Desktop already runs one reader
per window. Remaining Desktop reads either move into a part, stay as explicit
user-action reads, or are deleted.

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
statement counts are. PR #1692 covers the one known scan (agent_sessions by
execution owner).

An orphaned reader (pid 5210, from an exited Loopflow Dev.app) had run for 3h37m
at 99% CPU: a descendant still held its stdin open, which was the only stop
signal.

## This change

- The reader exits when its parent does (checked each heartbeat), not only when
  stdin closes. Verified in a real run: 0.14.15 still running 8 s after its
  parent is killed while a sibling holds stdin; the branch build exits after 1.5 s.
- A part's change- or clock-driven reading waits until `BUSY_SHARE` (5) × its last
  reading's duration has passed. Scope changes, refresh and checkpoint still
  read at once. From the timings above, planning on Jack's store drops from
  ~23 readings/min (~37 s busy) to ≤ 7.5 (~12 s); a change can show up to ~8 s
  late (from ~2.5 s). Unit-tested. Not A/B'd live: see check results.
- Desktop: WavesView took each Wave's Projects from one `lf wave status` per Wave
  on every planning frame. It now reads `planning.roadmap.waves[].projects`,
  which is the same `project_planning` result.
- Deleted `RegistryQuery.plan`, `waves(repoPath:)`, `userName`, `taskFiles`,
  `repositoryWaves` (no callers left) and their exclusive tests.

## Plan per read

Scope: M machine, R repo, W wave, T task. "Stream" = already a part.

| Read | Answers | Desktop caller / cadence | Scope | Place in the API | Work needed |
|---|---|---|---|---|---|
| planning (`wave show --all`, `wave list --all --current`) | roadmap, current Waves, Dashboard plans | stream | M | part | set-based Task details (below) |
| sessions (`session list --page`) | Session rows | stream (headless: false) | R | part | page or diff; drop 2 s blind clock once liveness is a signal |
| task (`task status`) | TaskOverview, Graph processes, comments | stream | T | part | N+1 over Graphs; 20 connections |
| task_history | TaskWork diagnostics | stream, on request | T | part | rename vs `usage --task` clash |
| wave (`wave status`) | Wave detail | stream | W | part | shares planning's Task details |
| work_activity (`history`) | 7-day activity | stream | W/P/T | part | |
| activity (`monitor top --json`) | live call trees | stream | M | part | shrinks with LOO-615 |
| peer_planning (`planning status`) | sync receipts | stream | R | part | |
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

1. Incomplete-Task scope (decided): planning, `wave show` and `wave status`
   select open Tasks only (`task_open_sql`); add the paged completed-Task read
   (`lf task list --done --page`, sketch) and move Desktop's Done views to it
   in the same change so nothing disappears. One cut, no flag.
2. Task search (decided): one read over incomplete Tasks matching title,
   identifier, Wave, Project and state, returning `RoadmapTask` rows. Index
   `tasks(repository_id)` partial on the open predicate plus the identifier;
   titles are short enough to scan once the set is ~200 rows. Sketch:
   `lf task search <text> [--wave] [--project] [--state] --json`, and a
   `search` field in the stream's scope request for live results.
3. Planning/Wave Task details: replace per-Task reads (`active_task_pr`,
   `task_state`, `completion_pending`, `task_ownership`, Graph execution) with
   one query per concern over the Tasks in scope. Target: statements per
   reading independent of Task count; p99 < 250 ms on Jack's store.
4. Sessions: incremental or paged rows instead of 5.7 MB per reading;
   replace the 2 s clock with the activity part's liveness change
   (`observe_quiet` already covers the time-based Waiting threshold).
5. `lf task status`: 5k statements / 20 connections for one Task.
6. Delete Desktop duplicates listed above; add catalog and machine parts.
7. Per-part durations in `heartbeat` so clients see p99 directly (shared with
   the performance-measurement Task).

## Delete — do not maintain

- Done: `RegistryQuery.plan/waves(repoPath:)/userName/taskFiles/repositoryWaves`,
  WavesView per-Wave `lf wave status`.
- Next: `taskComments` read, `allWaves` after createWave, no-stream-only reads
  (`roadmap`, `sessionPage`, `peerPlanningStatus`, `workActivity`) if the
  one-shot transport is not kept for remote Machines.

## Check results

- `cargo test -p loopflow --lib work_watch`: 29 passed. clippy/fmt clean.
- `scripts/test_desktop.sh --filter 'RegistryQueryTests|DTOFixtureTests'`: 63 passed.
- Live A/B of the rest: not possible here. Dev builds without drafts re-exec the
  installed lf on the shared home (`installation.rs`), and a cloned home has no
  writers. Deferred to gate/demo with an installed candidate.
