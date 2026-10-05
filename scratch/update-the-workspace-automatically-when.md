# Update the workspace automatically when Loopflow data changes (LOO-382)

Status: **partly built, 2026-10-05.** Jack Heart requested the Task and made the
decisions below in design review; the mechanism was proposed, not separately
accepted. Product decisions in the Task brief are binding. Open choices are in
[questions.md](questions.md).

## Remaining work

Built on the branch: the `store_revisions` migration and coverage test, the
change waiter, `lf monitor workspace --watch --json` with planning, Sessions,
Task work, Wave detail, Work activity and process activity parts, the
unfinished-Exec completion-gate read, and the Desktop cutover with every
`lf`-spawning timer loop deleted. Not yet done:

1. **Planning reading is about 2 s on a copy of Jack's store, not 300 ms.**
   Inside the retained process it was about 7.5 s before two changes: Git
   answers are reused briefly, and Exec membership is reached from the 109
   unfinished Execs. Samples were taken with the machine at load 50–60, so
   they are rough. What is left is mostly per-Task Session membership SQL and
   Git content reads. The follow-up Task Jack asked for on a miss is **not
   filed**.
2. **`active` is still its own process.** `monitor active --watch` was not
   folded in (question 5's fallback, taken for now); a window runs two readers.
   Both share one Swift pipe reader, `LocalLineObservation`.
3. **The explicit one-shot reads survive.** `PodiumModel.refresh`,
   `refreshPlanning`, `refreshSessions`, `refreshProcessActivity` and
   `refreshWorkActivity` still spawn `lf` when no reader is open: session
   fixtures, and about sixty call sites in four test suites. With a reader open
   they send a request instead. `roadmapGeneration` and `sessionsGeneration`
   remain to guard those reads. Moving the suites onto scripted frames deletes
   both.
4. **Git-only changes are slower than before.** Checkout contents are re-read
   at most once a minute and only when planning is read; with no store commit
   that is every five minutes. The 15 s poll used to show a dirty checkout
   sooner. Nobody chose this trade.
5. **No rendered measurement.** The write-to-visible scenario in
   `desktop_performance.py` and the installed-app demo are not built or run.
   The only latency observed is commit to frame in a debug build on a tiny
   store: 309 ms and 718 ms, 250 ms of it the burst window.
6. **Swift coverage gaps.** A scripted feed proves in-place updates, an older
   sequence ignored, a pre-write frame ignored, another repository's rows
   ignored, `refresh` waiting for its answer, and reader end then recovery. Not
   covered: a frame from another Home, a completed Task leaving the working
   set, and UI-test modes, which no longer re-read Sessions every 2 s.
7. **`WavesView`** (Portfolio and repository windows) now follows the stream
   through its own model; unexercised beyond compiling.

## Decisions (Jack Heart, 2026-10-05 design review)

- **One landing.** Slices 1–4 are built in order and delivered together.
- **Desktop is reactive to the store and nothing else.** Linear reaches the UI
  only through a sync committing to the store; the two are independent. How
  syncs are scheduled or triggered (a schedule, Desktop asking for one,
  webhooks — Jack's stated preference among these) is not this Task.
- **The latency budgets are targets, not a landing gate.** "Reasonable enough
  for now, but dont block landing on an unachieveable number": pursue them for
  about an hour of focused work; if they are not met, ship the measured result
  with its numbers stated and file a follow-up Task for the gap.

## Problem

Jack Heart creates a Task and the open Desktop sidebar does not show it. The
work succeeded but looks missing until something refreshes. The same staleness
applies to renames, completion, Session binding and Flow progress, whether the
change came from Desktop, `lf`, a background worker or another window.

## What the system does today (measured 2026-10-05, installed `lf` 0.13.3, Jack's Home)

Desktop has no change observation for planning or Sessions. `PodiumModel`
polls by spawning `lf`:

| Loop | Where | Cadence | Command |
| --- | --- | --- | --- |
| Planning | `PodiumModel.keepPlanningCurrent` | 15 s | `roadmap --all --json`, `wave list` |
| Sessions | `PodiumModel.keepSessionsCurrent` | 2 s | `session list --json --page` |
| Process activity | `PodiumView` `.task` | 2 s | `monitor ps --json` |
| Work activity | after each planning read | 15 s | `monitor activity` |
| Wave detail | `WaveDetailPane` | 30 s | `wave status` |

One streaming reader already exists: `lf monitor active --watch --json`
(`runs_watch.rs`, `LocalActiveRunsObservation.swift`) — newline frames, stdin
requests, latest-frame backpressure, FSEvents invalidation.

Findings that shape the design:

1. **Reads are far slower than their cadence.** `roadmap --all` 18.6–22.7 s
   (627 KB), `session list` 7.5–7.8 s, `wave list` 3.3 s, `home id` 2.3 s,
   `--version` 0.06 s. A new Task is visible 20–40 s after commit at best. A
   notification that merely triggers the same subprocess read would not help.
2. **Polling is the largest writer in the store.** Every `lf` read records an
   Exec. Of 65,043 Exec rows written in the last 24 h, ~63,000 are Desktop's
   own polls (`session list` 25,836; `monitor ps` 22,764; `wave list` 4,788;
   `roadmap --all` 4,781; `monitor activity` 4,772). `execs` holds 212,476
   rows; the database is 1.1 GB.
3. **That write volume is what makes the read slow.** A 12 s sample of
   `roadmap --all` puts ~70% of command time in `wave_tasks →
   snapshot_task_detail → task_completion_gate → associated_work_blockers →
   SqliteStore::task_work → read_exec`: one query per Task whose
   `exec_ids` selector joins `execs` on checkout path with no usable index, so
   each of ~340 Tasks scans the whole table. The read gets slower every day
   Desktop is open. Process admission (`journal::admit_process`) adds roughly
   1 s to every invocation.
4. **A naive "watch the database, then re-read" loops forever**, because the
   read is itself a commit.
5. **Cross-process commits are cheaply observable** (probe, WAL mode, separate
   writer processes): a kqueue vnode watch on `-wal` saw 20/20 commits; a
   200-row transaction produced one delivery; idle produced none; reading
   `data_version` plus a revision row costs 3.5 µs; a rolled-back write does
   not move a trigger-maintained revision. `data_version` signals change but
   does not count commits. A truncating checkpoint produced no write event, and
   the `-wal` file can be removed when the last connection closes, so the watch
   must re-arm from the directory. FSEvents latency for the same writes was not
   probed.
6. The roadmap projection also reads Git and the filesystem per Task
   (worktree blockers). Those are not store commits.
7. **Table-level change domains would not filter anything while agents work.**
   Last 24 h on Jack's Home (read-only count, 2026-10-05): 168,336
   `session_events`, of which 164,192 are provider transcript lines
   (`kind='observed'`, receipt key `…:events.jsonl:…`) and 3,021 are `usage`.
   Transcript lines arrived in 610 of the day's 1,440 minutes, up to 2,095 in
   one minute. Everything else in the table — lifecycle, manifest, terminal and
   driver-exit facts — is 1,338 rows, at most 26 in a minute. `flow_events`:
   214. Execs not written by Desktop's polls: 2,067. `kind` alone does not
   separate the two: lifecycle facts are also `observed`.

## Demo

With Desktop open on the Loopflow repository, sidebar scrolled, a draft typed in
a Session:

```bash
lf task create --wave product --title "Reactive demo"   # no --run
```

The Task row appears once under Product within a second. Nothing else moves:
selection, draft, pane layout and scroll stay put. Then, from the terminal:
rename it, bind a Session to it, complete it. Each change shows without touching
Desktop; the completed Task leaves the working set and is still found under
Completed. `scripts/benchmarks/desktop-performance` records write-to-visible
latency for each.

## Chosen approach

One long-lived reader per window, fed by store-owned change revisions.

### 1. The store says what changed (shared interface with Infrastructure)

A `store_revisions(domain TEXT PRIMARY KEY, revision INTEGER NOT NULL)` table,
bumped by SQLite triggers on insert/update/delete. Domains: `planning` (waves,
projects, tasks, task PRs, PM snapshots, metrics), `sessions` (agent sessions and
the Session events that change a displayed row), `flows` (flow sessions and
events), `execs`.

A domain follows what a write can change on screen, not which table it lands
in. Provider transcript lines and `usage` events bump nothing (finding 7): no
part of this stream displays them, and counting them would invalidate every
part for ten hours a day. The `session_events` trigger therefore carries a
`WHEN` predicate. Its exact form is a sketch — receipt key not matching
`:events.jsonl:` and kind not `usage` — to be settled against what
`session list` and the roadmap conditions read. Live transcript display is
LOO-293's; it adds its own domain when it has a reader for one.

Triggers live in the schema, so every writer bumps them — any `lf` version,
any future command — with no call-site discipline to forget. A rolled-back
write bumps nothing. A store test enumerates every table and fails unless it
is assigned a domain or listed as never displayed; the transcript exemption is
listed there by predicate, not by table.

A Rust `StoreChanges` reader holds one `query_only` connection and waits on
filesystem events for `loopflow.db-wal` (generalizing the existing
`session_record/active/events.rs` subscription, re-arming when the file is
recreated). On wake it reads the four revisions. A 30 s check and an explicit
`rescan` cover missed events, checkpoints and sleep/wake. Revisions are the
authority; events only say "look now".

### 2. One watch process projects and streams

`lf monitor workspace --watch --json`: one process per window, one Exec row for
its lifetime, reads on a `query_only` connection. It emits newline-framed
envelopes:

```json
{"part":"planning","sequence":41,"answers":7,"home":"…","revisions":{"planning":911,"flows":88,"sessions":402,"execs":5120},"body":{…RoadmapSnapshot…}}
```

Parts and what invalidates them:

| Part | Body (existing wire DTO) | Recomputed when |
| --- | --- | --- |
| `planning` | `RoadmapSnapshot` + Waves | `planning`, `flows`, `sessions`, `execs`; 5 min clock |
| `sessions` | Session records for the scoped repository | `sessions`, `flows`, `planning` |
| `task` | `TaskWork` for the selected Task | any domain |
| `wave` | `WaveDetailSnapshot` for the shown Wave | any domain; 5 min clock |
| `work_activity` | `WorkActivitySnapshot` for the selection | any domain |
| `activity` | `ActivitySnapshot` | `execs`; 2 s clock for liveness |
| `active` | `ActiveSessionsSnapshot` | not built: still `monitor active --watch` |

Bodies are the existing DTOs unchanged, so Swift keeps its decoders and
LOO-376's saved wire text keeps working. A part is emitted only when its bytes
differ from the last frame sent, so a write that changes nothing visible sends
nothing. Projections are coalesced: 100 ms trailing quiet capped at 250 ms from
the first unhandled bump, so a steady writer cannot postpone a projection
indefinitely; at most one projection per part in flight; newest frame replaces
an unsent one.

As built, "differs" ignores the fields that only restate when a reading was
taken (`generated_at`, `evidence_age_secs`, a condition's `observed_at`, the
activity window's bounds); revisions are re-read every second as the
missed-event check; and a request forces its parts to answer even unchanged.

Byte comparison hides wasted work: a projection that ran and produced the same
bytes sends nothing and still cost its 300 ms. The heartbeat frame therefore
carries per-part projection counts, so "idle" and "busy producing nothing" are
distinguishable in tests and in the field.

Stdin requests: `scope` (repository, headless filter, selected Task),
`refresh`, `rescan`. Each carries an id; every frame names the newest request
handled before its projection began (`answers`). A window that just ran a write
command sends `refresh` and waits for a frame answering it — that frame
necessarily reflects the write. This replaces the generation counters guarding
against stale poll results.

### 3. Make the planning projection cheap enough to run on change

Required, not optional: without it the stream delivers a 20 s old answer.
The completion gate in the roadmap list stops loading every associated Exec per
Task. The blockers only concern unfinished execution, so the list path reads
unfinished Execs once for all Tasks (or the membership query gets an index it
can use). Budget: full planning projection ≤ 300 ms p95 on Jack's Home inside
the retained process. If Git-backed blockers dominate after that, they move to
a slow clock with their last result retained, stated in the frame as observed
time, never as fresh.

### 4. Desktop consumes one stream

`PodiumModel` stays the window's single refresh owner. Its sleep loops become
one consumer of the workspace stream; `refresh()` becomes a `refresh` request.
Frames apply with the existing equality checks, so unchanged rows do not
re-render. Rules:

- Drop a frame whose `sequence` is older than the part's last applied frame, or
  whose `home` or scope differs from the current one.
- Never pass through `.loading` between frames; a stream failure shows the last
  good reading as unavailable and restarts with backoff, then `rescan`.
- Selection clears only when a complete planning frame omits the selected Task
  and it is not retained history; drafts and native terminals are untouched.
- Search filters and Show completed stay window-local over the same rows.
- A repository or Home switch sends `scope` and ignores frames until one
  answers it.
- Wake from sleep sends `rescan` (the hook exists for active Sessions).

## Alternatives considered

| Route | Verdict |
| --- | --- |
| Poll faster | Rejected. Reads take 8–23 s and each poll writes an Exec; faster polling makes both worse. |
| Refresh after Desktop's own Create | Rejected by the Task: misses `lf`, workers and other windows. |
| File-watch then spawn `lf` reads | Rejected. Still pays 2 s+ admission per read and self-triggers (finding 4). |
| Recompute everything on any commit, compare bytes | Rejected as the only filter. Exec noise during agent work would run the planning projection continuously. Byte comparison is kept as the last filter. |
| Writers bump revisions in Rust | Rejected. Hand discipline at every write site; older installed binaries would miss it. Triggers make the omission unspellable. |
| Swift reads SQLite directly | Rejected. A second schema reader beside `lf`; breaks the single-reader rule. |
| Darwin notifications from writers | Rejected. Same discipline problem; no catch-up after a missed post. |
| Restore a Home server | Rejected. Jack deleted listeners and `lfd` on 2026-09-27. The watch is a foreground child of the window and dies with it. |

## Contract

**User-visible outcome.** A committed change to Tasks, Waves, Sessions or Flow
state appears in every open window scoped to it, without action, within the
budget below.

**Source of truth.** Each Home's `loopflow.db`. `store_revisions` is
derived inside the same transaction. Frames, `PodiumModel` state and the saved
workspace are derived views; none is written back.

**Affected surfaces.** New: `lf monitor workspace --watch --json`, the frame
envelope DTO (`tests/fixtures/dto/workspace_frame.json`, Rust and Swift fixture
tests), one draft migration. Changed: `PodiumModel`, `RegistryQuery`,
`PodiumView`, `WaveDetailPane`, the roadmap completion-gate read. Unchanged:
existing `--json` reads for CLI users and one-shot Desktop reads (comments,
files, diffs, usage).

**Absent and error states.**

| Situation | Behavior |
| --- | --- |
| No store yet | Frames with empty bodies; the watch waits for the file to appear. |
| Projection fails | Part frame carries the existing `Unavailable` evidence; Desktop keeps last good rows, marked. |
| Stream ends or stalls past the heartbeat | Last good reading shown as unavailable; restart with backoff; no silent freeze. |
| Missed event, checkpoint, sleep | 30 s revision check or `rescan` converges. |
| Record deleted | Next complete frame omits it; it is not resurrected from saved text. |
| Liveness not verified | Stays unknown; a frame never upgrades it. |
| Home or installed `lf` replaced | Watch exits on configuration change as the active reader does; the window reopens it and drops other-Home content. |

**Operational budget.** Targets to pursue, time-boxed per Jack's decision; the
spawn, Exec-row and no-polling rows are structural and are not negotiable.

| Measure | Now | Target |
| --- | --- | --- |
| `lf task create` commit → row rendered | 20–40 s | ≤ 1 s p95 |
| Session bind/complete → row updated | 8–10 s | ≤ 500 ms p95 |
| `lf` spawns per idle window | ~75 / min | 0 from refresh paths |
| Exec rows written by an idle window | ~63,000 / day | 1 per window launch |
| Idle watch CPU | n/a | < 1% of a core |
| Planning projection in the watch | 18–23 s as a command | ≤ 300 ms p95 |

**Exclusions.** No Linear observation in Desktop (Jack's decision above): a
change made in Linear appears when a sync commits it, and then by the same
path as any other commit. Git-only changes (commits, dirty files) keep their existing slower
paths. Remote-Home streaming over SSH is not proven here. LOO-380 owns
reconciling stale live records. No change to process admission cost for
one-shot commands. iOS is unchanged.

**Delete — do not maintain.**

- Deleted: `PodiumModel.keepPlanningCurrent`, `keepSessionsCurrent` and their
  `Task.sleep` cadences; the 2 s `refreshProcessActivity` loop in `PodiumView`;
  the 30 s loop in `WaveDetailPane`; `WavesView.pollRegistry` (the view is
  routed, so it stays and follows the stream).
- Still to delete: `roadmapGeneration` / `sessionsGeneration` and the one-shot
  bodies of `refresh`, `refreshPlanning`, `refreshSessions`,
  `refreshProcessActivity`, `refreshWorkActivity` (remaining work 3). Do not
  extend them.
- Still to delete: the separate `monitor active --watch` process, once
  `active` is a part of the workspace stream. Its native-receipt reader,
  bounded frames and explicit-retry behavior survive.
- Must survive: saved-workspace launch (LOO-376), page-wise Session history on
  request, `updateTaskDirective`'s "absent after accept" error, fixture mode.

**Forbidden outcomes.** A polling fallback left beside the stream. A second
mutable cache of planning in Swift or Rust. Frames with a Desktop-only shape
that drifts from `--json`. Any refresh path that writes an Exec per read. A
revision bump implemented at call sites. Activity shown from a frame whose
liveness was not observed.

## Internal slices

1. **This slice — store revisions and the watch's planning part.** Migration
   with triggers and the table-coverage test; `StoreChanges`; `monitor
   workspace --watch` emitting `planning`; the completion-gate read fix.
   Focused test: `cargo test -p loopflow --test workspace_watch`.
2. Desktop cutover for planning: stream consumer, request/answer ordering,
   delete the planning loop. `swift test --filter PodiumModelTests`.
3. `sessions` and `task` parts with scope requests; delete the Session and
   Wave-detail loops.
4. `activity` and `active` parts; delete the process-activity loop and the
   second watch process.
5. Rendered benchmark scenario and the installed-app demo.

Slices 1–4 land together (Jack's decision); they are ordered for building,
not for separate delivery.

## Done when

- `cargo test -p loopflow --test workspace_watch` passes, covering: a Task
  created by a second process appears exactly once in the next planning frame
  within 2 s; 200 writes across 20 transactions yield a final frame equal to a
  fresh one-shot read and no more than 5 frames; an Exec insert alone yields no
  planning frame; 2,000 transcript-line inserts over 10 s run zero projections
  of any part (heartbeat counts, not absence of frames); a Task created during
  that transcript stream still appears within 2 s; a writer committing every
  50 ms to `planning` gets a frame within 500 ms, not after it stops; a
  stopped-then-continued watch and a recreated `-wal` both
  converge; after `scope` no frame for the previous repository is emitted;
  5 idle seconds emit no part frames and add no Exec rows.
- `cargo test -p loopflow store_revisions` passes: every table has a domain or
  an explicit exemption; rollback does not bump; upgrade from the released
  frontier creates the triggers.
- `swift test --filter PodiumModel` passes with a scripted feed: selection,
  drafts and Session panes survive frames; an older `sequence` and a
  wrong-Home frame are ignored; no `.loading` appears between frames; a
  completed Task leaves the working set and remains under Completed; stream
  end shows last good as unavailable and recovers.
- `cargo test -p loopflow --test dto_fixtures` and `swift test --filter
  DTOFixtureTests` cover the frame envelope.
- `uv run python scripts/desktop_performance.py run` gains a write-to-visible
  scenario; 20 comparable samples are recorded against the budget table, with
  failed attempts retained. A miss after the time-boxed pursuit is reported
  with its numbers and a filed follow-up Task, not held back from landing.
- Demo/review (people, not gate): the create → rename → bind → complete
  sequence on the installed app on Jack's Home, with evidence limits stated.

Toward the chapter KRs: three working days and three two-hour sessions on
Desktop are not credible while successful work looks missing and every read
costs seconds. This plan earns no KR by itself.

## Risks

- **Trigger coverage drifts** when a table is added. The coverage test is the
  guard; the 30 s check does not help, because it reads the same revisions.
- **The transcript predicate can be wrong in either direction.** Too wide and a
  displayed Session state stops updating, with nothing to catch it but the 30 s
  check — which reads the same revisions and will not. Too narrow and the
  stream reprojects continuously. A test per displayed Session field, written
  against a second-process writer, is the guard for the first; the heartbeat
  projection counts for the second.
- **`execs` invalidates planning.** Cheap once Desktop's polls are gone (2,067
  a day). One landing means no interval in which a surviving poll loop bumps
  it; during development, expect the planning projection to re-run on every
  remaining poll until slice 4.
- **FSEvents latency on `-wal`** is unmeasured; the probe used kqueue. The
  integration test decides; kqueue on the file plus a directory watch is the
  alternative.
- **The 300 ms projection budget** rests on one sample attributing ~70% to the
  Exec scan. The remainder (PM portfolio validation, metrics, Git) may need
  the same treatment.
- **A long-lived process holds one binary** across an `lf` upgrade. It must
  exit on configuration change, as the active reader does.
- **Migration on a 1.1 GB store** adds triggers only, no backfill; cost should
  be schema-only but is unmeasured.

## Check results

- 2026-10-05 probe: 20/20 cross-process commits observed, 1 delivery per
  200-row transaction, 0 idle events, 3.5 µs check.
- 2026-10-05 implement: `cargo test -p loopflow --test workspace_watch` 5/5;
  `--test dto_fixtures` 19/19; `--lib store_revisions` 4/4; `--lib task_work`
  pass; `cargo clippy --all-targets -- -D warnings` clean; `swift test --filter
  "PodiumModel|SessionControls|SessionRename|ActiveSession|WorkspaceNavigation|RegistryQuery"`
  pass after one wording fix; `check_migrations.py` and
  `check_architecture.py` pass. `status_tests`
  `previous_release_merge_request_migrates…` fails locally because it applies
  published migrations only; it needs `materialize_rust_tests.py`, left to gate.
