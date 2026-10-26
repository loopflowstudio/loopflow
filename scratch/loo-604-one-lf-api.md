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
- Desktop: WavesView reads `planning.roadmap.waves[].projects` instead of one
  `lf wave status` per Wave. Deleted dead `RegistryQuery` reads.

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

1. Graph execution is still per Task with a Graph: `graph_entry`,
   `process_input` (recursive CTE) and process evidence. Batch over the
   reading's latest Graphs. Tasks without Graphs cost nothing per Task now.
2. Task search (decided): `lf task search <text> [--wave] [--project]
   [--state] --json` over open Tasks, returning plan rows, plus a `search`
   field in the stream's scope request. No index planned: open Tasks are ~230
   rows and are scanned once per reading.
3. Sessions: incremental or paged rows instead of 5.7 MB per reading;
   replace the 2 s clock with the activity part's liveness change.
4. `lf task status`: 5k statements / 20 connections for one Task; reuse
   `TaskDetailFacts` for its Task.
5. Delete Desktop duplicates listed above; add catalog and machine parts.
6. Per-part durations in `heartbeat` so clients see p99 directly.

## Delete — do not maintain

- Done: `RegistryQuery.plan/waves(repoPath:)/userName/taskFiles/repositoryWaves`,
  WavesView per-Wave `lf wave status`.
- Next: `taskComments` read, `allWaves` after createWave, no-stream-only reads
  (`roadmap`, `sessionPage`, `peerPlanningStatus`, `workActivity`) if the
  one-shot transport is not kept for remote Machines.

## Check results

- `cargo test -p loopflow --lib work_watch`: 29 passed (first slice).
- `scripts/test_desktop.sh --filter 'RegistryQueryTests|DTOFixtureTests'`: 63 passed (first slice).
- `cargo test --test integration -- planning_statements closed_task_pages lifecycle_world`: 3 passed (open-Task slice; statements flat 2 → 6 Tasks).
- `cargo test --lib -- waves task_ownership planning_sync children`: 53 passed; `chapter_deletion_stays_absent…` passes once its surviving Task is started rather than completed.
- `scripts/test_desktop.sh --filter 'TaskHistoryFilterTests|DTOFixtureTests|RegistryQueryTests'`: 72 passed (Done view paging).
- `docs/lf-reference.md` regenerated for `task list --done/--after/--limit`; `reference_matches_the_compiled_cli` passes.
- Live p99 A/B with an installed build: deferred to gate/demo.
