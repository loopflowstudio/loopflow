# LOO-1110: an idle `lf server` does no work

## Outcome

With nothing changing on disk or in the store, `lf server run` sits under 1% of
one core, under 20 threads, and forks nothing. Under 13 running Graphs it stays
under 5%, and a held terminal keeps echoing keystrokes while the store is
locked. Temp-home servers and holders end by themselves. Ten Tasks building at
once share the cores instead of each taking all of them.

One design rule carries every piece: **work happens on a change or a request,
never on a timer that finds nothing to do**, and each process holds one of each
expensive thing (store connections, runtime, observation cache).

## What the Machine showed (2026-11-25, 16-core M4 Max)

Brief's profile: server at ~80% average CPU over 3h26m, ~180 threads, 9 forks/s.
Later comment on the Task: "launchctl print recorded 38,287 forks (~2.8/sec
sustained), and its single held-terminals thread became completely unresponsive
(every held.sock client timed out with os error 35 until server restart). Keep
an eye on held-terminals thread resilience/non-blocking behavior alongside the
CPU/SQLite/YAML/git-polling fixes."

A second 3 s `sample` of the same server (PID 77346, 77 threads, 15% CPU at the
time), taken before it was restarted:

- `held-terminals` thread: **2,214 of 2,314 samples** in `transport::hosted::Hosted::pass
  → store::opening::open_sqlite → configure_write_connection → sqlite3InitOne`.
  The one thread that relays every terminal opens a store inside its loop.
- `plan-git` thread: 721 samples in `plan_git::exchange_unowned`, 263 of them
  `store::ownership::machine_names → placement::global_settings_at →
  observe::derived → resolve_with → filter_file_preferences → validate_value`
  (one full `Config` deserialization per key, uncached because the thread has
  no `READING`).
- Schema: 113 tables, 259 indexes, **402 triggers carrying 283 KB of SQL**
  (`sqlite_master`). Every fresh connection parses all of it. A cold read-only
  open plus first statement measured 3.9 ms median on an idle Machine (Python
  `sqlite3`, 20 runs); a warm query 2 µs.
- Thread names: 41 `tokio-rt-worker`, 4 `notify-rs fsevents loop`, 23 unnamed.
- 24 `lf holder run` processes from worktree builds, 1–3 days old. **21 have an
  `LF_HOME` that no longer exists** (`/private/tmp/lf-ios-*`, `lf-two-*`,
  `lf-wake-e2e-*`, `/var/folders/.../.tmp*`), each keeping one child (`sh`,
  `cat`, `opencode`, `pi`, `claude`). 3 belong to homes that exist.
- `launchctl list` showed no `com.loopflow.server.<hash>` job at that moment;
  the brief found three earlier.

## History

`observe` has been patched for this twice in three weeks: #1868 (shared reader
for identical demand, −97% CPU for 16 identical subscribers) and #1979 (derived
settings cache; `validate_value` 4,915 → 12 samples, server still at 104% and
"now SQLite"). Both left the timers and the per-request opens. This is the
third pass, so it redesigns the refresh rule and the store handle instead of
adding another cache beside them.

## Design

### 1. Observation refreshes what someone holds; the rest is checked on use

Today (`rust/loopflow-execution/src/observe.rs`): three threads wake every
250 ms and re-observe every answered fact on its interval, held or not, for
300 s after its last use. One-shot served reads fill the same cache
(`server/service.rs:1874`), so a single Desktop read of a Task diff leaves
`git diff`, `git ls-files` and one `git diff --no-index` per untracked file
re-running every 3 s for five minutes. `derived()` keeps nothing without a
`READING` (`observe.rs:590`).

Becomes:

- **One rule for every fact, the one `CheckoutFiles` already has**
  (`observe.rs:322`, `:424`): a fact is refreshed in the background only while
  a `Dependencies` holds it. An unheld fact is re-observed inline when it is
  read and older than its interval. Unheld entries still leave after 300 s.
- **One refresh thread** that sleeps until the next held fact is due and parks
  when nothing is held. The three 250 ms loops go.
- **`File`/`Bytes`/`Directory` compare `(mtime, size, inode)` before reading
  content.** External edits keep arriving within 1 s for held facts
  (`read_stream.rs` test `configuration_changes_reach_an_existing_subscription_without_store_writes`).
- **Held `Git` and `CheckoutFiles` facts re-run after a filesystem event under
  their checkout, at most once per 3 s**, through the `notify` watcher
  `ops/task/workspace.rs` (`TaskFilesWatch`) already uses. A checkout nobody is
  editing forks nothing. Linux keeps the 3 s timer for held Git facts: `notify`
  there adds one inotify watch per directory, and `target/` alone exceeds the
  default limit. Passed over: `gix` in-process Git. With idle forks at zero the
  remaining forks are real diffs after real edits, and `gix` would be a second
  Git implementation whose output must match byte for byte.
- **`derived()` uses the process cache with or without a `READING`.** Background
  loops (`plan_git`, `tracker_events`, recovery, wake) get the same
  `Arc<Config>` served reads get; a hit costs the dependency check (a few
  `stat`s at most once a second). `Outside::shared` keeps bypassing the cache
  under a `--setting` override.
- `settings::outside()` runs `which_on_path` for every harness before the cache
  key exists (`config.rs:165-180`). The PATH scan result is kept per
  `(PATH, harness)` for the process.
- `transcript::find` caches hits only (`loopflow-harness/src/transcript/mod.rs:143`);
  a missing Codex transcript is re-walked on every Session-list reading. Misses
  are remembered until the transcript watcher reports a create under that home.
- The four `notify-rs` threads (one per harness home, `transcript/watch.rs`)
  become one watcher with four roots.

### 2. Settings validate once per file, not once per key

`filter_file_preferences` (`config/settings.rs:465`) calls `validate_value` per
key, and each call builds `catalog::defaults()` and deserializes a whole
`Config` (`settings.rs:226-229`). Becomes: overlay the whole layer on the
defaults and deserialize once. Only when that fails does the per-key walk run,
to find which keys to drop and warn about. A valid file costs one
deserialization; an invalid one costs what it does today and reports the same
keys. Passed over: hand-written per-field validators, a second statement of
every type `Config` already declares.

### 3. A process keeps its store connections

`open_sqlite` is called from inside library code everywhere (`screens.rs`,
`desktop.rs`, `waves.rs`, `answers.rs`, `boundary.rs`, `ops/recovery.rs`,
`plan_carriers.rs:191`, `rental/recovery.rs`, 30 s `watch_owned_work`), so one
served Desktop read opens three stores and one agent hook opens two. Fixing
call sites in `server/*.rs` would miss most of them.

Becomes: `open_sqlite(path)` and `open_read_only` hand out a connection the
process already has. `loopflow-storage` keeps idle connections per database
identity `(canonical path, dev, inode)`, write and read-only separately, a few
each. Dropping the last clone of a `SqliteStore` returns its connection.

- Validation (migration frontier, schema comparison) runs when a connection is
  first opened and again when `PRAGMA schema_version` differs from the value it
  was validated at. A server newer than its store keeps being refused; a store
  migrated under a running server is revalidated on the next use.
- A connection that comes back mid-transaction is closed, not kept.
- Trace hooks are re-armed on every hand-out (`instrumentation::observe_sqlite`
  reads the request-tracing flags at open today).
- `open_ephemeral*` and private-home migration opens are not kept. Keying on
  file identity keeps a deleted and re-created test home from getting the old
  connection; `TEST_LEDGER_DB_PATH` and `LF_HOME` are still resolved per call.
- `open_read_only_store` (`store/mod.rs:540`) opens twice to validate then
  read; it becomes one hand-out.
- Statement cache capacity goes from rusqlite's default 16 to 256, and static
  SQL in `loopflow-storage/src/sqlite/` uses `prepare_cached` (178 `.prepare(`
  today, 23 cached). `query_row`/`execute` sites move only where the benchmark
  ranks them.
- The store commits at the end of every pinned reading as today; kept read
  connections hold no transaction between uses, so they don't pin the WAL.

Passed over: a server-only pool behind a new handle type. The opens are in
shared command code that the CLI also runs, so the fix belongs under the
function they all call.

### 4. One runtime, counted threads

- `read_stream::reader_runtime()` builds a default multi-thread runtime **per
  subscription** (`read_stream.rs:458`): 16 workers each on this Machine. All
  Readers share one small runtime.
- 72 `Runtime::new()` in `lf/commands` and 35 in `loopflow-execution` (three
  per recovery settle, one per served read in `screens.rs:185`,
  `read.rs:1381`). They go through one process `block_on` on a shared
  current-thread-capable runtime.
- The server runtime (`server/mod.rs:40`) sets `worker_threads` and
  `max_blocking_threads`; today it is one worker per core and up to 512
  blocking threads. Start at 4 workers and 12 blocking and let the saturation
  regressions (`subscriptions.rs` cancellation and executor-saturation tests)
  say whether 12 holds.
- `maintenance` and `plan-git` become one periodic thread; `recovery` and
  `task-update-wake` both wait on `StoreChanges` and become one.
- `plan_carriers::after_command` (`plan_carriers.rs:203`) spawns a thread and
  opens a store per served request before its 5 s debounce check. The debounce
  moves in front of the thread.
- `plan_git::git_until` spawns two pipe-reader threads per Git command; it
  reads both pipes on the calling thread with `wait_with_output`-style polling.

### 5. Subscriptions step when their revisions move

`StoreChanges::wait` returns every 50 ms for 1 s after any write
(`loopflow-storage/src/changes.rs`), and the store observer calls `look()` on
every follower each time (`read_stream.rs:794-835`). With agents writing
constantly, every subscription steps at 20 Hz forever. The observer reads
`revisions()` once per wake and wakes only followers whose domains moved.
Followers with held facts are woken by the refresh thread when a fact changes.

### 6. The held-terminal thread never touches the store

`transport/registry.rs` runs one thread for every held terminal's sockets and
PTY relay. `Hosted::pass` reads agent state from the store
(`hosted.rs:571-575`) and `settle(exit)` writes exits from the same loop
(`registry.rs:864`). A slow open or a busy write lock stops every terminal on
the Machine. Store reads and writes move to the existing `held-status` worker
over its channel; the loop only sends and drains. Pass 1 names the exact
`open_sqlite` site under `Hosted::pass` from a symbolized sample before moving
it.

### 7. The minute exchange covers repositories with work in them

`plan_git::exchange_unowned` walks every `repository_plans` row
(`ops/plan_git.rs:44`): 70 on this Machine, 54 of them temp or demo paths, and
`plan_carriers::before_first_read` registers any clone with a remote on any
read. The loop takes repositories that have an unfinished Task or a Wave on
this Machine; a repository with neither exchanges on its next command
(`before_first_read`, `after_command`), as it does today. No row is deleted
and no path is classified as temporary. A repository whose exchange fails
doubles its wait up to an hour.

### 8. A custom-home server is an ordinary process

Every `LF_HOME` other than `~/.lf` registers `com.loopflow.server.<hash>` with
`KeepAlive` (`installation/server.rs:66-165`), and nothing ever boots one out.
A demo, QA or test home therefore leaves a daemon that respawns forever, keeps
fetching, and restarts agents through its recovery pass.

Chosen for review: **custom homes are never registered with launchd or
systemd.** `ensure()` starts their server the way `server/linux.rs:41-91`
already does without a user manager: a detached `lf server run`. That server
exits when its home directory is gone, or after 10 minutes with no client, no
held terminal and no unfinished Graph or agent. The next launch or `lf server
connect` starts it again. The default home keeps SMAppService and systemd
unchanged.

- Cost: a custom home's conversations don't come back after a reboot until
  something runs `lf` there. Custom homes are development and test homes
  (`docs/dev/machines.md:676`).
- `ensure()` for a custom home boots out that home's old label when loaded, so
  existing registrations retire on first use. A registered server that starts
  and finds its home missing boots out its own label (`XPC_SERVICE_NAME`)
  before exiting, which ends today's crash loops.
- Passed over: an "ephemeral" flag or temp-path detection (a list to extend),
  and tying the server to its launcher's PID (the launcher exits while its
  Sessions continue).

**A holder ends when its home is gone.** `terminal_holder.rs` exits only after
60 s with no child and no reader. 21 of 24 lingering holders serve a deleted
home: no server can ever adopt them and their child waits on a PTY forever. A
holder whose `LF_HOME` no longer exists closes its PTYs (its own children get
SIGHUP from the kernel) and exits. Holders of an existing home are unchanged:
they outlive servers and installs (`docs/dev/server-first.md:242`).
`MachineServer::drop` in `loopflow-test-support` keeps killing only its server;
fixtures delete their home, and the holder follows.

`.lf/qa/hooks.md:69` and `.lf/qa/held-terminals.md:17` lose their "boot that
label out" instructions.

### 9. Graph steps share the cores

Nothing limits concurrent builds: each step's agent runs `cargo` with one job
per core, and `scripts/test.py` caps only its own invocation
(`performance-budgets.json`: 4 jobs, nice 10).

Chosen for review: `RunningGraph::spawn` (`lf/commands/graph.rs:979`), the one
place every step starts, sets a job budget in the step's environment:
`cores ÷ running Graph steps on this Machine`, floor 2, exported as
`CARGO_BUILD_JOBS`, `NEXTEST_TEST_THREADS`, `MAKEFLAGS=-jN` and `LF_JOBS`, and
starts the step at nice 5. Thirteen Graphs on 16 cores give each build 2 jobs:
at most 26 compiler processes where the brief saw load 144. A value the
person already set wins. New setting `machine.jobs` overrides the division.

- Passed over: a slot semaphore that queues builds. lf does not see which
  agent command is a build, and a queue that holds an agent's tool call is lf
  in the middle of the turn. A budget in the environment needs no cooperation
  and cannot stall anything.
- Passed over: deferring suites to CI under load. `gate` already publishes
  first and runs few tests locally.
- The count is read when the step starts; a step that starts alone keeps its
  larger budget when others join. Left that way.

### 10. Bounded files

Runs in the hourly maintenance pass, default home and custom:

- `logs/lf.log` (197 MB, never rotated): rotate at 32 MB, keep 4.
- `logs/terminal-*.log`, `logs/graph-*.log` (9,333 files): delete those
  untouched for 30 days whose Session or Graph has ended.
- `context-deliveries/*.json` (24,265): delete those older than 7 days whose
  launch has ended. `.thread` and `.store-gap` markers stay; they carry state.
- `loopflow.db.backup-*` (730 MB): `settle_migration_backups` waits for every
  superseded `lf` to exit, which with week-old holders never happens. The
  backup goes once the frontier it preceded has been in use for 7 days.
- Lock files (`loopflow.session-locks/` 27,594, `loopflow.admission/`,
  `creation-locks/`, `human-sessions/`): a file is removed only while this pass
  holds its exclusive lock and its owner (Session, request) is ended. A holder
  that then creates a new file at that path locks a different inode than a
  waiter on the old one, so removal re-checks the owner under the lock and
  takers re-stat after locking (`fstat` inode equals path inode, else retry).

## Exclusions

- **`processes` (271 MB with indexes), `planning_peer_changes`,
  `git_tracker_changes`**: already marked leaving under LOO-965 and LOO-938;
  nothing has written an `lf` process row since 2026-11-18. Their staged drops
  reclaim the space. Deleting 165k rows here would fire per-row triggers under
  the shared write lock for no lasting gain.
- **Schema size.** 402 triggers make each cold open slow; with kept
  connections it is paid once per process. Collapsing triggers is its own
  staged migration work.
- **Tracker polling in every Graph agent step** (`watch_owned_work` at 30 s per
  process). Kept connections make each pass cheap; moving it into the server
  is a separate change.
- Removing dead `repository_plans` rows, in-process Git, Linux filesystem
  events for Git facts.
- Stopping the leaked processes on this Machine today. After this ships the
  21 orphan holders are still the old build; they are stopped by PID, by hand.

## What goes

- `observe.rs`: the three per-source refresh threads and their 250 ms sleep;
  background refresh of unheld facts; the `READING`-only branch of `derived`.
- `settings.rs`: per-key `validate_value` in the file-read path (the function
  stays for `lf config set`).
- `read_stream::reader_runtime`; the scattered `Runtime::new()` in command and
  ops code; `block_on_task`'s runtime construction (`ops/task.rs:429`).
- `store/mod.rs` `open_read_only_store`'s validate-then-reopen.
- `installation/server.rs`: plist and unit generation for custom homes, hashed
  labels, the legacy `com.loopflow.machine.<hash>` retirement (folded into the
  one bootout), and the ignored launchd fixture in `tests/transport_tests.rs:2669`
  with `scripts/test_server_registration.py`'s custom-home cases.
- `maintenance.rs` `start_plan_exchange` as its own thread; `wake.rs`'s own
  watcher thread.
- Tests pinned to the old timing are rewritten to the new rule, not loosened:
  `transcript/watch.rs:113-155` (1100 ms sleep), `observe.rs` `refresh_all`'s
  61 s backdate.

Must survive: external config edits reach a live subscription within 1 s;
process death, deadlines and the clock invalidate held readings without a
store write (`observe.rs:996-1048`, `:1220-1312`); a server newer than its
store refuses; holders outlive server replacement in a live home; per-request
SQL receipts in subscription frames; `leaving_guard` in test stores.

## Acceptance check at gate

New `scripts/benchmarks/server_idle.py`, beside `subscription_baseline.py`,
against a release build in a private home seeded with 12 repositories, 13
Tasks with checkouts, 64 Sessions and a 113-table store:

```sh
cargo build --release -p loopflow
uv run python scripts/benchmarks/server_idle.py target/release/lf --seconds 60
```

It holds one client with a board subscription and one Task-detail
subscription open, then reports three phases. Budgets live in
`scripts/performance-budgets.json`; the script exits non-zero past one.

| Phase | CPU (one core) | Threads | Forks | Store opens |
|---|---|---|---|---|
| quiet, 60 s | < 1% | < 20 | 0 | 0 |
| one file edited per second in one checkout | < 3% | < 20 | ≤ 1 per 3 s | 0 |
| writer process committing 20 writes/s | < 5% | < 24 | 0 | 0 |

CPU and threads come from `ps`; forks from `getrusage(RUSAGE_CHILDREN)` deltas
plus a spawn counter; opens from `loopflow-storage/src/instrumentation.rs`,
read through `lf server status --json`, which gains `threads`, `store_opens`,
`spawns` and `held_facts` so the same numbers are visible on a live Machine.
The base commit's numbers go in the PR body next to the branch's.

Focused tests that fail if the feature breaks:

- `observe`: an unheld Git fact is not re-run after its interval; a held one
  re-runs after a write under its checkout and not before.
- `settings`: a file with one unknown key keeps its neighbours and warns once,
  with one `Config` deserialization for a valid file (counter in `cfg(test)`).
- `loopflow-storage`: two sequential `open_sqlite` calls yield one
  `Connection::open`; a schema change between them revalidates; a store behind
  the binary is still refused on a kept connection.
- registry: with another connection holding the write lock for 5 s, a byte
  typed into a held terminal echoes within 200 ms.
- installation: `ensure()` in a custom home registers no launchd job and the
  server exits after its home is removed; a holder exits after its home is
  removed and its child receives SIGHUP.
- graph: two Graph steps started together on a 16-core fixture each see
  `CARGO_BUILD_JOBS=8`; a preset value is left alone.

After merge, on the live Machine: `sample <server pid> 3` shows no
`serde_yaml_ng`, `sqlite3InitOne` or `posix_spawn` frames with Desktop open and
13 Graphs running, and `lf server status --json` reports under 20 threads.

## Demo

```sh
lf server status --json | jq '{threads, store_opens, spawns, held_facts}'
top -pid "$(pgrep -f 'lf server run' | head -1)" -stats cpu,th -l 5
```

With Desktop open on a Task and thirteen Graphs running: CPU reads 0.x, 18
threads, `spawns` unchanged across two calls a minute apart. Then
`LF_HOME=$(mktemp -d) lf task create … --run`, delete the directory, and
`ps` shows that home's server and holder gone within a minute, with
`launchctl list | grep com.loopflow.server.` empty.

## Passes

1. **Measure, then stop the store and settings storms.** `server_idle.py`, the
   status counters, base numbers recorded. Kept connections (§3), one-shot
   settings validation and always-on `derived` (§1 last three bullets, §2),
   held-terminal thread off the store (§6). This pass can prove the plan
   wrong: if kept connections cannot hold across `TestLedgerGuard` homes and
   schema changes, or the quiet phase stays above 5% after it, the remaining
   passes change shape. It shows the quiet-phase store opens at 0 and the
   `sample` free of `validate_value` and `sqlite3InitOne`.
2. **Held-only observation and one runtime.** §1's refresh rule and watcher,
   §4, §5, §7. Shows forks at 0 quiet and threads under 20.
3. **Lifetimes.** §8, with its docs (`docs/dev/server.md` "Installation owns
   registration", `machines.md`, the two QA notes).
4. **Job budget and bounded files.** §9, §10, `machine.jobs` in
   `docs/config.md`.

Each pass deletes what it replaces and leaves the benchmark passing its
then-current budgets.

## Checks

- `uv run python scripts/benchmarks/server_idle.py target/release/lf --seconds 60` — not run yet; implement runs it on base and branch.
- `sample 77346 3` on the live server, 2026-11-25 — run at kickoff, results above.
