# Update the workspace automatically when Loopflow data changes (LOO-382)

Status: **stream, the five planning-read slices and second-round slices 6–9
built, 2026-10-05; slices 10 and 11 dropped with evidence. Not shipped; the
window was not re-measured.** Jack Heart requested the Task and made the
decisions below. Open choices are in [questions.md](questions.md).

## Simple, fast planning read (direction 2026-10-05)

Jack Heart, after the walkthrough: seeing which Waves and Tasks are current is
a basic part of the UX and the architecture should make it very simple and
fast; each thing moves to its right owner, Swift or the database; build on the
performant, scalable shape from the start.

Measured on a snapshot copy of Jack's store (1.75 GB, debug `lf`, load 32):

- The current plans hold 140 Tasks in 45 Waves; 80 Tasks have local work, 32
  a checkout. 30 of the Waves are dead test repositories.
- A warm planning reading in the watch took 2.8–6.6 s. Sampled, 83% of it is
  one statement: the completion gate's unfinished-Exec membership query, run
  once per unfinished Task. Everything else in the reading is about 300 ms.
- A first reading took 16 s and a one-shot `wave list` 4.9 s, half of it in
  the kernel: Git subprocesses per Wave and per checkout.

So the list is slow because of one per-Task query and cold Git, not because
it carries each Task's detail. Rows in the Wave Task list show condition,
status and next owner, and the working set of a finished Task depends on
observed Git state, so a list of bare columns would change what is shown.

**The rule.** A reading of all current Waves and Tasks runs a fixed number of
statements, never one per Task, and never reads finished history. Git is
asked only about checkouts that exist. Under that rule re-reading every Task
on a change costs milliseconds per hundred Tasks, so the revision counters stay
the invalidation and the wire and Swift types do not change.

Not built, and why: a light list type beside per-Task detail (the detail is
cheap once read in bulk, and splitting it changes many views); stored derived
rows and a per-row change log (the next step when a bulk reading stops fitting
its budget; nothing here blocks it). Jack said he wants derived state in the
database; this plan stores none (question 23).

### Measuring

A snapshot copy of Jack's store with this branch's schema is at
`/private/tmp/claude-501/-Users-jack-src-loopflow-update-the-workspace-automatically-when/4ec913e9-a411-4564-b44d-282182f16a26/scratchpad/jhome`
(made with `VACUUM INTO`, draft applied with `sqlite3`). It is disposable;
never point a development `lf` at `~/.lf`. After changing the draft's
indexes, recreate them on the copy by hand. Use a release build: a debug
build compiles SQLite unoptimized.

```bash
cargo build --release -p loopflow   # <lf> is the absolute path of target/release/lf
python3 scratch/measure/warm.py <home> <lf> [sample.txt]   # six refreshes, request to answering frame
python3 scratch/measure/cold-sample.py <home> <lf> sample.txt 12   # first reading, sampled
python3 scratch/measure/flame.py sample.txt flame.html "<title>"   # the sample as a flame chart
python3 scratch/measure/hot.py sample.txt 40 80           # hot frames by sample count
python3 scratch/measure/commit.py <home> <lf> 5            # sqlite3 renames a Task, then a Session; writer's exit to the frame
python3 scratch/measure/checkout.py <home> <lf> <clean Task checkout> 5   # a file written, then removed, to the frame
python3 scratch/measure/usage.py <home> <lf> 30            # usage rows every 0.2 s; readings per part
python3 scratch/measure/idle.py <home> <lf> 30 workspace   # or active: CPU and memory left alone
python3 scratch/measure/oneshot.py <home> <lf> <cwd> 10    # planning readings per one-shot read
```

Always record `uptime` beside a number; this machine is rarely quiet.

### Slices (all five built, 2026-10-05)

Numbers are from the copy with a release `lf`; load is the one-minute average.

1. **One membership read.** `SqliteStore::open_execs` pairs every unfinished
   Exec with its Tasks in one statement; the gate takes each Task's Execs
   from it. The index and join order are pinned, because the bundled SQLite
   otherwise scans every Exec (350 ms against 23 ms).
2. **Cold reading: 4.2–4.6 s** at load 17–30 (five starts, one 7.0 s), against
   6.5–7.1 s. The reading does not ask Git about a Wave's repository or a
   Task's checkout that is gone (`wave_main_repo`, `task_reference`). The
   same test inside `git_common_dir` changed no timing, since Git fails fast
   there, and was removed in compress; other callers still start Git for a
   missing directory. What is left, sampled:
   about 1.7 s of Git on the 32 checkouts that exist, five commands each, one
   after another; 0.9 s process admission; 0.6 s of foreign-key checking that
   a development build runs twice on every open. Not built: asking Git about
   the checkouts at once. A launch shows saved text first (LOO-376), so this
   wait is the first refresh, not the first frame.
3. **Warm reading: 0.25–0.28 s** request to answering frame at load 20 (six
   refreshes; 0.28–0.32 s at load 26), against 2.8–6.6 s. Timed inside the
   reader once: the planning projection is 190–200 ms (roadmap 180, Wave
   list 12), and a back-to-back refresh waits about 70 ms behind the process
   reading before it. **Commit to frame, five samples at load 15–19: a
   renamed Task 301–327 ms, a renamed Session 121–131 ms**, each including
   the 100 ms quiet wait. Slice 1 alone met the 300 ms projection target, so
   nothing else was changed for it.
   **The rule is not fully met.** The gate still runs statements per
   unfinished Task: its Sessions and Flows (`members`, about 30 ms of a
   reading and the one read that still walks a Task's finished Sessions),
   then per Session its driver, row and pending turn, and per Task its
   status, Flow and PRs. Reading them in bulk means restating the completion
   rule over open Sessions only, in the code that also decides `lf task
   complete`. Not built: the reading fits its budget, and that rewrite risks
   the authority for about 40 ms (question 25).
4. **A failing part backs off.** It is read again after 1 s, 2 s, 4 s … up
   to 60 s (`PartState::retry_due`). A commit to what it reads, or a request,
   still reads it at once. Unit-tested; not run against a failing store.
5. **Rust owns the working-set rule.** `TaskConditionSnapshot` carries
   `unresolved_execution`: the Task is ready, its Flow is not idle, or its
   checkout holds observed unsettled work. Swift's
   `TaskHistoryFilter.hasUnresolvedExecution` is deleted; `includes` takes
   the Task and its condition. Whether a Task has open Sessions, and
   "started", stay in Swift: they join Session rows the window holds.

### Second round (Jack Heart, 2026-10-05, after the first five slices)

Jack decided: keep optimizing while clear, meaningful wins remain; remove the
obvious performance taboos now; fix both lags nobody chose; the outline keeps
started Tasks only; no follow-up Tasks are needed. Each slice below is
implemented, compressed and realigned in turn, measured with the recipe
above, and dropped with its evidence if it turns out not to be a clear win.

6. **Cold reading: built.** A reading in the watch first asks Git about
   eight existing checkouts at a time (`ask_checkouts_ahead`), so the Task
   details find the answers retained. First reading **1.4–2.3 s at load
   32–36 (five starts), 1.1–1.2 s at load 23 (two)**, against 4.2–4.6 s.
   Sampled after: the asking is under 0.1 s; what Git is left is the
   completion gate's ancestry check on a few Tasks (about 0.27 s at load 30,
   arguments come from each PR). Left alone. The store-open foreign-key
   check is gone on main (`store/MIGRATIONS.md`), and `lf home id` on this
   build takes 0.25 s, so admission is not 1.4 s here; neither was touched.
   An installed build was not measured. One-shot commands retain nothing
   and ask in turn as before.
7. **Git-only changes: built.** The watch follows each existing Task
   checkout and, for a linked worktree, its Git metadata directory, with one
   FSEvents stream (`engine/fs_events.rs`, the binding the active-Session
   reader already used, moved and shared). A changed checkout is asked about
   again at most every 3 s (`reread_retained`); planning and Wave detail are
   read again only when an answer differs, so a build writing ignored files
   costs four Git commands per 3 s and no reading. **A file written in this
   Task's checkout reached the planning frame in 2.0–2.7 s, and its removal
   in 1.3–2.2 s, at load 46 (five each, `measure/checkout.py`)**, against one
   to five minutes. The watch's Git reads set `GIT_OPTIONAL_LOCKS=0`. The
   clocks are unchanged. macOS only; elsewhere the clocks still stand.
   CPU of the watch while a checkout builds is unmeasured.
8. **Token totals: built.** A fifth revision, `usage`, moves on each usage
   event or `events.jsonl` usage line (one insert trigger). Only Wave detail
   follows it, and a change that is usage alone waits until the part was
   last read 10 s ago. **145 usage rows written over 30 s to the copy read
   Wave detail 3 times and planning 0 times** (load 25, `measure/usage.py`),
   against a lag of up to five minutes. `StoreRevisions` gained the field on
   the wire, in Swift and in the frame fixture. The cost of one Wave-detail
   reading on Jack's store is unmeasured. The copy's draft was re-applied by
   hand (table recreated, trigger added).
9. **Task-files comparison: built, in Swift.** The 10 s loop is deleted.
   The view already watched the checkout; `TaskFileObservation` now also
   watches a linked worktree's Git metadata directory, and a change to
   `HEAD`, `index` or a ref re-reads the comparison. The recorded PR base
   is part of the view's reading identity, so a store commit that moves it
   re-reads too. A window showing a comparison spawns `lf` on change, not 6
   times a minute. Swift test: a `HEAD` rewritten in a linked worktree's
   metadata re-read the comparison in 0.19 s. Not exercised in the window.
   It does not use the Rust watch of slice 7: both watch the same two
   directories, each for its own reader. Found on the way: on a temporary
   volume the checkout's own FSEvents paths never matched (`/private`
   prefix); real checkouts are unaffected and it was left alone.
10. **One reader per window: dropped.** `monitor active --watch` stays its
    own process. Measured on the copy at load 9–12 (`measure/idle.py`, 30 s):
    it starts in 0.23 s, idles at 0.2% of a core and holds 25 MB, and a
    window starts it only on first Monitor demand. Folding it in saves that
    and one Exec row per window. It costs rewriting the reader's
    demand-start, explicit-retry and drain-on-Home-change lifecycle in Swift,
    and moving it from the writable store it opens to the watch's read-only
    one. Limit: the copy has no `runs/` receipts, so its idle cost on Jack's
    Home is higher than measured. Not a clear win against a proven reader.
11. **One-shot reads: dropped.** Measured (`measure/oneshot.py`, load 15):
    ten `lf session list` beside a running watch caused 20–22 planning
    readings, about 0.2 s each in the reader, and no frame for reads in a
    Task checkout (one frame in the run from the main checkout, cause not
    traced). The rule as planned cannot be written in a trigger: an insert
    does not know the Exec will finish at once, and Desktop's remaining
    reads (file, diff, comments) run inside the Task checkout, where a live
    Exec is a real completion blocker that planning shows. Exempting them
    needs the store to know a command only reads, which is a change to
    Infrastructure's Exec model (questions 6 and 9). After slice 9 a window
    makes these reads only on a click.

Also measured: the workspace reader left alone on the copy uses 1.6% of a
core and 37 MB (target under 1%). Unprofiled; the 2 s process observation is
the likely cost.

Not in this round: reading the completion gate's remaining per-Task
statements in bulk (about 40 ms of a 200 ms reading, in the code that
decides `lf task complete`; not a clear win), and stored derived rows.

## Remaining work

1. **The outline keeps started Tasks only** (Jack Heart, 2026-10-05). A Task
   created without a Run shows in its Wave's Task list at once and in the
   outline once a Run or Session is recorded. LOO-382's first acceptance
   line is not met as written, by that decision.
2. **The window has not been re-measured.** Reader numbers are in the slices.
   The rendered benchmark (`desktop_performance.py write-visible`) last ran
   before slice 1: 3 samples, debug builds, a renamed or created Task about
   2.3 s warm against 1 s, a Session change 550–600 ms against 500 ms. The
   20-sample release run and the installed-app demo are gate's and demo's.
   A development `lf` reads a Home copy only after the draft is applied to
   the copy by hand.
3. **`active` is still its own process** (slice 10, dropped). Both readers
   share one Swift pipe reader, `LocalLineObservation`.
4. **The explicit one-shot reads survive.** `PodiumModel.refresh`,
   `refreshPlanning`, `refreshSessions`, `refreshProcessActivity` and
   `refreshWorkActivity` still spawn `lf` when no reader is open: session
   fixtures and about 140 call sites in twenty test suites. With a reader
   open they send a request. `roadmapGeneration` and `sessionsGeneration`
   guard those reads. Moving the suites onto scripted frames deletes both.
5. **Swift coverage gaps.** A scripted feed proves in-place updates, an
   older sequence ignored, a pre-write frame ignored, another repository's
   rows ignored, `refresh` waiting for its answer, reader end then recovery,
   another Home's frame dropping the previous Home's Sessions and selection,
   and a Task completed elsewhere staying under Completed and selected. Not
   covered: UI-test modes, which read Sessions once; `WavesView` following
   the stream beyond compiling; slice 9 in a window.
6. **Rust coverage.** `workspace_watch` covers a Task committed elsewhere,
   bursts, each displayed Session fact, a store created later, a store that
   cannot be opened, a steady writer, a paused reader, scope, a checkout
   changed on disk, and usage read only by Wave detail. Short of the
   original list: the transcript test writes its 2,000 lines in about a
   second, not ten; Task binding is not walked; Work activity and Wave
   history have no per-fact test.
7. **The copy's draft is hand-applied.** The upgrade path through `lf` with
   the final draft, including the `usage` domain, is covered by the
   migration tests only.

## Decisions (Jack Heart, 2026-10-05 design review)

- **One landing.** Slices 1–4 are built in order and delivered together.
- **Desktop is reactive to the store and nothing else.** Linear reaches the UI
  only through a sync committing to the store; the two are independent. How
  syncs are scheduled or triggered (a schedule, Desktop asking for one,
  webhooks — Jack's stated preference among these) is not this Task.
- **The latency budgets are targets, not a landing gate.** "Reasonable enough
  for now, but dont block landing on an unachieveable number": pursue them for
  about an hour of focused work; if they are not met, ship the measured result
  with its numbers stated. He first asked for a follow-up Task for the gap;
  in the second round he said none are needed.

## Problem

Jack Heart creates a Task and the open Desktop sidebar does not show it. The
work succeeded but looks missing until something refreshes. The same staleness
applies to renames, completion, Session binding and Flow progress, whether the
change came from Desktop, `lf`, a background worker or another window.

## What the system did before (measured 2026-10-05, installed `lf` 0.13.3)

Five loops in `PodiumModel` and its views spawned `lf` every 2–30 s. The
full table and findings are at commit `018083656`. What still shapes the
design:

- Reads were slower than their cadence (`roadmap --all` 18.6–22.7 s), and
  about 63,000 of 65,043 daily Exec rows were those polls. That volume made
  the read slow: one unindexed Exec scan per Task.
- A reader that commits wakes itself.
- A kqueue watch on `-wal` saw 20/20 commits from other processes; a
  truncating checkpoint produced no event and the file can be removed, so
  the watch re-arms and a 1 s re-read of the revisions is the authority.
- Table-level domains filter nothing while agents work: 164,192 of 168,336
  daily `session_events` were transcript lines. `kind` alone does not
  separate them from lifecycle facts.

## Demo

The first step shows the row in the Wave's Task list, not the outline
(remaining work 1).

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
events), `execs`, and `usage` (slice 8).

A domain follows what a write can change on screen, not which table it lands
in. Provider transcript lines bump nothing: no part of this stream displays
them, and counting them would invalidate every part for ten hours a day.
Usage bumps only `usage`. The `session_events` trigger therefore carries a
`WHEN` predicate. As built it exempts kind `usage`, and kind `observed` with
`:events.jsonl:` in the receipt key whose evidence type is one the summary
readers already skip (`activity`, `handoff`, `user_input`, `conversation`,
`text`, `tool_use`, `result`, `provider_output`) or `usage`. The first cut
exempted every `:events.jsonl:` row. That also silenced provider attempts and
identities, about 1,270 rows a day, which Work activity and a Wave's Session
history read. `session list` and the roadmap conditions read only captured,
manifest, driver-exit, started and completed events, none exempt. Live
transcript display is LOO-293's; it adds its own domain when it has a reader
for one.

Triggers live in the schema, so every writer bumps them — any `lf` version,
any future command — with no call-site discipline to forget. A rolled-back
write bumps nothing. A store test enumerates every table and fails unless it
is assigned a domain or listed as never displayed; the transcript exemption is
listed there by predicate, not by table.

A Rust `StoreChanges` reader holds one `query_only` connection and waits on
filesystem events for `loopflow.db-wal` (generalizing the existing
`session_record/active/events.rs` subscription, re-arming when the file is
recreated). On wake it reads the revisions. A 1 s re-read and an explicit
`refresh` cover missed events, checkpoints and sleep/wake. Revisions are the
authority; events only say "look now".

### 2. One watch process projects and streams

`lf monitor workspace --watch --json`: one process per window, one Exec row for
its lifetime, reads on a `query_only` connection. It emits newline-framed
envelopes:

```json
{"part":"planning","sequence":41,"answers":7,"home":"…","revisions":{"planning":911,"flows":88,"sessions":402,"execs":5120},"unavailable":null,"body":{…RoadmapSnapshot…}}
```

Parts and what invalidates them:

| Part | Body (existing wire DTO) | Recomputed when |
| --- | --- | --- |
| `planning` | `RoadmapSnapshot` + Waves | `planning`, `flows`, `sessions`, `execs`; a changed checkout; 5 min clock |
| `sessions` | Session records for the scoped repository | `sessions`, `flows`, `planning` |
| `task` | `TaskWork` for the selected Task | any domain |
| `wave` | `WaveDetailSnapshot` for the shown Wave | any domain, `usage` at most every 10 s; a changed checkout; 5 min clock |
| `work_activity` | `WorkActivitySnapshot` for the selection | any domain |
| `activity` | `ActivitySnapshot` | `execs`; 2 s clock for liveness |
| `active` | `ActiveSessionsSnapshot` | dropped: stays `monitor active --watch` |

Bodies are the existing DTOs unchanged, so Swift keeps its decoders and
LOO-376's saved wire text keeps working. A part is emitted only when its bytes
differ from the last frame sent, so a write that changes nothing visible sends
nothing. Projections are coalesced: 100 ms trailing quiet capped at 250 ms from
the first unhandled bump, so a steady writer cannot postpone a projection
indefinitely; at most one projection per part in flight; newest frame replaces
an unsent one.

As built, "differs" ignores the fields that only restate when a reading was
taken (`generated_at`, `evidence_age_secs`, a condition's `observed_at`, the
activity window's bounds), and a request forces its parts to answer even
unchanged.

Byte comparison hides wasted work: a projection that ran and produced the same
bytes sends nothing and still cost its 300 ms. The heartbeat frame therefore
carries per-part projection counts, so "idle" and "busy producing nothing" are
distinguishable in tests and in the field.

Stdin requests: `scope` (repository, headless filter, selected Task) and
`refresh` (also sent on wake; a separate `rescan` did the same thing and is
gone). Each carries an id; every frame names the newest request
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
  whose scope differs from the current one. A frame from another Home drops
  the previous Home's content and selection, then applies (question 21).
- Never pass through `.loading` between frames; a stream failure shows the last
  good reading as unavailable and restarts with backoff, then `refresh`.
- Selection clears only when a complete planning frame omits the selected Task
  and it is not retained history; drafts and native terminals are untouched.
- Search filters and Show completed stay window-local over the same rows.
- A repository or Home switch sends `scope` and ignores frames until one
  answers it.
- Wake from sleep sends `refresh` (the hook exists for active Sessions).

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
| Store exists and cannot be opened | Every store part is unavailable with the reason, retried after 1 s doubling to 60 s (slice 4). Found on a Home copy at another schema, which read as an empty workspace. |
| Projection fails | Part frame carries the existing `Unavailable` evidence; Desktop keeps last good rows, marked. |
| Stream ends or stalls past the heartbeat | Last good reading shown as unavailable; restart with backoff; no silent freeze. |
| Missed event, checkpoint, sleep | 1 s revision re-read or `refresh` converges. |
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
path as any other commit. Remote-Home streaming over SSH is not proven here. LOO-380 owns
reconciling stale live records. No change to process admission cost for
one-shot commands. iOS is unchanged.

**Delete — do not maintain.**

- Deleted: `PodiumModel.keepPlanningCurrent`, `keepSessionsCurrent` and their
  `Task.sleep` cadences; the 2 s `refreshProcessActivity` loop in `PodiumView`;
  the 30 s loop in `WaveDetailPane`; `WavesView.pollRegistry` (the view is
  routed, so it stays and follows the stream).
- Still to delete: `roadmapGeneration` / `sessionsGeneration` and the one-shot
  bodies of `refresh`, `refreshPlanning`, `refreshSessions`,
  `refreshProcessActivity`, `refreshWorkActivity` (remaining work 4). Do not
  extend them.
- Deleted in the second round: the 10 s comparison loop in `TaskFilesView`.
- Kept by decision of slice 10: the separate `monitor active --watch`
  process.
- Must survive: saved-workspace launch (LOO-376), page-wise Session history on
  request, `updateTaskDirective`'s "absent after accept" error, fixture mode.

**Forbidden outcomes.** A polling fallback left beside the stream. A second
mutable cache of planning in Swift or Rust. Frames with a Desktop-only shape
that drifts from `--json`. Any refresh path that writes an Exec per read. A
revision bump implemented at call sites. Activity shown from a frame whose
liveness was not observed.

## Retired to git

Alternatives considered, the build-order slices and the full "Done when"
list are at commit `018083656`. The acceptance is unchanged; where the branch
falls short of it, the gap is in Remaining work.

## Risks

- **Trigger coverage drifts** when a table is added. The coverage test is the
  guard; the 1 s re-read does not help, because it reads the same revisions.
- **The transcript predicate can be wrong in either direction.** Too wide and a
  displayed Session state stops updating, and the 1 s re-read reads the same
  revisions, so nothing catches it. Too narrow and the stream reprojects
  continuously. A second-process test per Session fact guards the first for
  `sessions`; Work activity and Wave history have no such test, and their
  first exemption was too wide. The heartbeat projection counts guard the
  second and are tested. The trigger now parses each inserted payload's JSON;
  its cost on large transcript lines is unmeasured.
- **`execs` invalidates planning.** Every `lf` command anywhere costs two
  planning readings of about 0.2 s in each open window's reader and no
  frame (slice 11, dropped). 2,067 non-poll Execs a day on Jack's Home.
- **A checkout change the watch misses** waits for the clocks: Git answers
  stand for a minute and planning is read without a commit every five. The
  watch covers checkouts that existed when the planning revision last
  moved; a main checkout that is a Task's worktree also reports every
  write under its `.git`, which costs Git commands and no reading.
- **Two triggers now parse each inserted Session event's JSON**, one for
  `sessions` and one for `usage`. Unmeasured on large transcript lines.
- **The planning reading grows with unfinished Tasks.** 190–200 ms for 140
  Tasks, because the gate still runs statements per Task (slice 3). It fits
  the budget on Jack's store today; nothing bounds it.
- **A long-lived process holds one binary** across an `lf` upgrade. It must
  exit on configuration change, as the active reader does.
- **Migration on the 1.4 GB store** took 0.19 s on a copy, applied with
  `sqlite3`, not through `lf`'s upgrade path.

## Check results

Earlier passes are at commit `018083656`.

- 2026-10-05 second round: `cargo test -p loopflow --test workspace_watch`
  10/10; `--test dto_fixtures` 19/19; `--lib -- revisions workspace_watch
  store_revisions` 10/10; `--lib -- lf::commands::waves` 14/14; clippy
  `--all-targets -D warnings` clean; `check_migrations.py` pass; `swift
  test --filter "TaskFiles|DTOFixture"` pass. `status_tests`
  `previous_release_merge_request_migrates…` needs
  `materialize_rust_tests.py`, left to gate. Gate owns the rest.
- 2026-10-05 compress: clippy `--all-targets -D warnings` clean; `cargo test
  -p loopflow --test workspace_watch` 10/10; `--lib -- workspace_watch
  engine::git` 30/30. Not re-measured; gate owns the suites.
- 2026-10-06 compress: the reader's request, output and burst-wait loops are
  named functions; the Task-files comparison flag is a local. Clippy
  `--all-targets -D warnings` clean; `--test workspace_watch` 10/10; `swift
  test --filter TaskFiles` 19/19. Remaining work 4 untouched: about 140 test
  call sites in twenty suites still use the one-shot reads.
- 2026-10-05 sync: `cargo test -p loopflow --lib accepted_historical_uncertainty_completes_without_releasing_execution_protection` 1/1; bulk unfinished-Exec reads retain main's completion acceptance and Session-history API.
- 2026-10-06 realign after compress and the merge of main at `1a0dd6330`
  (v0.13.5, parallel `wt list` requests): the merge had no conflicts and
  changes nothing the plan relies on; every function, constant and deletion
  the plan names is in the tree. `cargo check -p loopflow --all-targets`
  clean; `check_migrations.py` pass against v0.13.5. Gate owns the suites.
