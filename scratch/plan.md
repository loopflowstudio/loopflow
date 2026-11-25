# LOO-1110: an idle `lf server` does no work

## Outcome

With nothing changing on disk or in the store, `lf server run` sits under 1% of
one core, under 20 threads, and forks nothing. Under 13 running Graphs it stays
under 5%, and a held terminal keeps echoing keystrokes while the store is
locked. Temp-home servers and holders end by themselves. Ten Tasks building at
once share the cores instead of each taking all of them.

One design rule carries every piece: **work happens on a change or a request,
never on an idle timer**, and each process holds **one** of each shared
resource (one store connection pool, one bounded async runtime, one observation
cache with one filesystem watcher). The terminal relay thread owns terminal I/O
only and never touches SQLite. Only the default Machine (`~/.lf`) is an
OS-registered service; custom `LF_HOME` servers and holders live only as long
as their home and active work do.

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
  Root cause verified in source (`hosted.rs:570-582`, `journal/mod.rs:753-758`):
  `lf server` has no `ProcessContext`, so `journal::process_ledger()` silently
  falls back to `open_ledger()` → `open_sqlite()` on every PTY output chunk
  until `execution_directory_reported` becomes true, AND `registry.rs:864`
  calls `settle(exit)` synchronously inside the single relay loop.
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
"now SQLite"). Both left the timers, the unpooled opens, the per-subscription
runtimes, and the permanent custom-home registrations. This pass redesigns the
runtime ownership across those boundaries instead of adding another cache beside
them.

## Design

### 1. Observation refreshes what someone holds; the rest is checked on use

Today (`rust/loopflow-execution/src/observe.rs`): three threads wake every
250 ms and re-observe every answered fact on its interval, held or not, for
300 s after its last use. One-shot served reads fill the same cache
(`server/service.rs:1874`), so a single Desktop read of a Task diff leaves
`git diff`, `git ls-files` and one `git diff --no-index` per untracked file
re-running every 3 s for five minutes. `derived()` keeps nothing without a
`READING` (`observe.rs:590`). Meanwhile `transcript/watch.rs` spawns four
separate `notify-rs` threads and `ops/task/workspace.rs` spawns per-checkout
watchers.

Becomes:

- **One rule for every fact, the one `CheckoutFiles` already has**
  (`observe.rs:322`, `:424`): a fact is refreshed in the background only while
  a `Dependencies` holds it. An unheld fact is re-observed inline when read and
  stale. Unheld entries still leave after 300 s.
- **One refresh thread** that sleeps until the next held fact is due and parks
  completely when nothing is held. When any held fact's revision bumps, it
  wakes only the subscriptions holding that fact (no polling in `read_stream`).
  The three 250 ms loops go.
- **One shared filesystem watcher** owned by `observe` (one FSEvents thread for
  the whole process, replacing the four `transcript/watch.rs` threads and
  sharing checkout watches with `TaskFilesWatch`):
  - Watches the four harness transcript homes on startup.
  - Adds/removes checkout roots as held `Git`/`CheckoutFiles` (and
    `TaskFilesWatch`) facts come and go.
  - On macOS, held `Git` and `CheckoutFiles` facts re-run after a filesystem
    event under their checkout, throttled to at most once per 3 s. A checkout
    nobody is editing forks nothing. Linux keeps the 3 s timer for held Git
    facts: `notify` there adds one inotify watch per directory, and `target/`
    alone exceeds the default limit. Passed over: `gix` in-process Git. With
    idle forks at zero the remaining forks are real diffs after real edits, and
    `gix` would be a second Git implementation whose output must match byte for
    byte.
- **`File`/`Bytes`/`Directory`/`Metadata` compare `(mtime, size, inode)` before
  reading content.** Held file facts keep arriving within 1 s for external
  edits (`read_stream.rs` test
  `configuration_changes_reach_an_existing_subscription_without_store_writes`)
  with zero content reads when unchanged.
- **`derived()` always uses the process cache, with or without a `READING`.**
  Background loops (`plan_git`, `tracker_events`, `recovery`, `wake`) and bare
  commands get the same `Arc<Config>` served reads get.
  - **Freshness guarantee for unheld dependencies:** because unheld facts are
    no longer background-polled, validating a cached `Computed` value whose
    dependencies are not held checks its `File`/`Bytes`/`Directory`/`Metadata`
    dependencies inline by `stat(mtime, size, inode)` (~3–5 µs) before serving
    the cached `Arc<T>`. An external edit written 1 ms before the next
    `load_config()` is seen immediately — never stale for 300 s or 1 s.
  - `Outside::shared` keeps bypassing the cache under a `--setting` override.
- `settings::outside()` runs `which_on_path` for every harness before the cache
  key exists (`config.rs:165-180`). The PATH scan result is kept per
  `(PATH, harness)` for the process.
- `transcript::find` caches hits only (`loopflow-harness/src/transcript/mod.rs:143`);
  a missing Codex transcript is re-walked on every Session-list reading. Misses
  are remembered until the shared watcher reports a create under that home.

### 2. Settings validate once per file, not once per key

`filter_file_preferences` (`config/settings.rs:465`) calls `validate_value` per
key, and each call builds `catalog::defaults()` and deserializes a whole
`Config` (`settings.rs:226-229`).

Becomes:
- Run the cheap structural + scope walk once (dropping `RETIRED_SETTINGS`,
  unknown keys, and out-of-scope keys without deserializing).
- Overlay the remaining top-level preferences onto `catalog::defaults()`
  **once**, deserialize `Config` **once**, and run the shared post-parse
  invariant checks (`agent`/`default_agent`/`agents`, `claude.model`, `stream`,
  `machine.tasks` vs `machine.capacity`, `approvers`, `ondemands`, `context`,
  `graph` — extracted from `validate_value` so both paths enforce identical
  rules). Do the same once per targeted `skills.<name>` /
  `graphs.<name>.steps.<name>` block when present.
- Only when that single pass fails does the per-key walk run, to find which
  keys to drop and warn about. A valid file costs one deserialization; an
  invalid one costs what it does today and reports the exact same warnings.
- Passed over: hand-written per-field validators, a second statement of every
  type `Config` already declares.

### 3. One process-wide SQLite connection pool

`open_sqlite` is called from inside library code everywhere (`screens.rs`,
`desktop.rs`, `waves.rs`, `answers.rs`, `boundary.rs`, `ops/recovery.rs`,
`plan_carriers.rs:191`, `rental/recovery.rs`, 30 s `watch_owned_work`, and
`journal::process_ledger()` inside `lf server` where `ProcessContext` is
`None`), so one served Desktop read opens three stores and one agent hook opens
two. Meanwhile `recovery`, `wake`, and every open `read_stream::Reader` hoard
their own private connections.

Becomes: `open_sqlite(path)` and `open_read_only` borrow warm connections from
a process-wide pool in `loopflow-storage`, keyed by database identity
`(canonical path, dev, inode)` and mode (write vs read-only), keeping a small
idle set of each.

- **What is pooled:** `(Connection, HistoryFile, validated_stamp)` — **never**
  `token_key` (which varies per caller, as tested in `opening.rs:70-121`). Each
  hand-out attaches the caller's `token_key` onto the returned `SqliteStore`.
- **Safe return on drop:** `SqliteStore` holds a pool lease whose `Drop` checks
  `Arc::try_unwrap` on `conn` (safe even when `test-support` clones `pub conn`);
  if the connection is mid-transaction (`!conn.is_autocommit()`) or another
  clone of `conn` escaped, it is closed instead of returned.
- **Validation that catches DDL and data-only migrations:** on checkout, check
  `PRAGMA data_version` (<1 µs shm read). If unchanged since this connection
  last validated, no external writer committed anything and validation is
  skipped. If `data_version` changed, check `(PRAGMA schema_version,
  schema_migrations count & max rowid)` (<5 µs) and run full frontier/schema
  validation only when either differs. A server newer than its store keeps
  being refused; a store migrated (DDL or data-only) under a running server is
  revalidated on next use.
- **Hooks and guards:** `instrumentation::observe_sqlite` is re-armed on every
  hand-out; `leaving_guard` stays installed on test stores.
- **Not pooled:** `open_ephemeral*` and private-home migration opens. Keying on
  `(canonical path, dev, inode)` prevents a deleted and recreated test home
  from ever receiving an old connection; `TEST_LEDGER_DB_PATH` and `LF_HOME`
  are resolved per call.
- `open_read_only_store` (`store/mod.rs:540`) stops opening twice to validate
  then read; it becomes one validated read-only hand-out.
- `read_stream::Reader` borrows a read-only connection from the pool for the
  duration of `step()` and returns it before sleeping, so 50 open subscriptions
  share a handful of warm read-only connections (and page caches) instead of
  pinning 50 connections in RAM.
- Statement cache capacity goes from rusqlite's default 16 to 256, and static
  SQL in `loopflow-storage/src/sqlite/` uses `prepare_cached` (178 `.prepare(`
  today, 23 cached). `query_row`/`execute` sites move only where the benchmark
  ranks them.
- Kept read connections hold no transaction between uses (`ReadSnapshot` drops
  at the end of `step()`), so they never pin the WAL.

Passed over: a server-only pool behind a new handle type. The opens are in
shared command code that the CLI also runs, so the pool belongs under the
function they all call.

### 4. One bounded runtime, counted threads

- **One async runtime per process:** `server/mod.rs:40` builds the server's
  bounded multi-thread runtime (start at 4 workers and 12 blocking threads;
  validate against `subscriptions.rs` cancellation and executor-saturation
  tests) and registers its `tokio::runtime::Handle` as the process's shared
  async handle (`runtime::install(Handle::current())`).
- `read_stream::reader_runtime()` (which today builds a 16-worker runtime **per
  subscription**, `read_stream.rs:458`) and the 107 scattered `Runtime::new()`
  calls in `lf/commands` and `loopflow-execution` go through one shared
  `runtime::block_on` helper:
  - Uses `Handle::try_current()` when already inside a blocking task on the
    Tokio runtime, else the installed server `Handle`, else a single lazily
    created small multi-thread runtime (2 workers) for bare CLI processes.
  - Never uses a shared `current_thread` runtime (which cannot run concurrent
    `block_on` calls safely) and never builds a runtime per subscription or
    request.
- `maintenance` and `plan-git` merge into one periodic thread; `recovery` and
  `task-update-wake` both wait on `StoreChanges` and merge into one thread.
- `plan_carriers::after_command` (`plan_carriers.rs:203`) debounces (5 s quiet
  window) *before* spawning a task or opening a store.
- `plan_git::git_until` reads stdout/stderr without spawning two extra
  pipe-reader threads per Git command.

### 5. Subscriptions sleep until their domains or held facts move

Today two loops wake idle and active subscriptions needlessly:
1. `StoreChanges::wait` (`loopflow-storage/src/changes.rs`) returns every 50 ms
   for 1 s after *any* write, and `read_stream::observe` calls `inbox.look()`
   on every follower each time (`read_stream.rs:794-835`) — stepping every
   subscription at 20 Hz under load.
2. `Reader::step()` schedules `wake = now + CHECK` (`CHECK = 1 s`,
   `read_stream.rs:668`) and `read_stream::observe` waits `CHECK` (`1 s`,
   `read_stream.rs:812`) — waking every idle subscription once per second
   forever even when nothing changed.

Becomes:
- The single store observer reads `revisions()` once per wake and wakes only
  followers whose watched domains (`Watch::State` vs `Watch::History`) actually
  moved since their last notified revision.
- `observe`'s single refresh thread pushes wakeups directly to the followers
  holding any fact whose revision bumped (`File`, `Git`, `Turn`, `Process`,
  `Deadline`, `Clock`, `RawConversations`).
- With both store and external-fact wakeups pushed directly, the fallback
  reconcile timeout (`CHECK`) moves from 1 s to 30 s (matching
  `TURN_RECONCILE`), so an idle subscription truly sleeps.

### 6. The held-terminal thread never touches SQLite or transcript files

`transport/registry.rs` runs one thread for every held terminal's sockets and
PTY relay. Today:
- `Hosted::pass` (`hosted.rs:570-582`) calls `journal::process_ledger()` →
  `open_sqlite()` → `agent_state()` synchronously on every PTY output chunk
  until `execution_directory_reported` becomes true (96% of samples on the live
  server, freezing `held.sock` with `os error 35`).
- `registry.rs:864` calls `settle(exit)` synchronously inside the poll loop,
  which runs `native_exit::settle()` (reading transcript files from disk and
  writing SQLite) and `end_terminal()`.

Becomes:
- Move both directory confirmation and exit settlement onto the existing
  `held-status` worker thread over its channel; the relay loop only sends
  requests and drains completions.
- Directory confirmation is requested at most once per terminal on first output
  (not per chunk), and the worker sends `Command::DirectoryConfirmed(terminal,
  cwd)` back on the command channel.
- **Preserving exit-code visibility without blocking:** while an exit is in
  flight on the `held-status` worker, the relay loop keeps its `TerminalId` in
  a `settling` set and answers `Answered::Waiting` on `held.sock` (just like
  pending holders) until the worker commits and sends
  `Command::Settled(terminal)`. Every other terminal keeps relaying keystrokes
  with zero delay; an attaching view of the just-ended terminal waits cleanly
  until its exit code is in the store.

### 7. The minute exchange covers repositories with work in them

`plan_git::exchange_unowned` walks every `repository_plans` row
(`ops/plan_git.rs:44`): 70 on this Machine, 54 of them deleted/temp/demo paths,
and `plan_carriers::before_first_read` registers any clone with a remote on any
read.

Chosen for review: the minute loop takes only existing directories that have an
unfinished Task or a non-retired Wave on this Machine; a repository with
neither still exchanges on its next command (`before_first_read`,
`after_command`), as it does today. No row is deleted and no path is classified
as temporary. A repository whose exchange fails doubles its wait up to an hour.

### 8. A custom-home server is an ordinary process

`docs/dev/machines.md:682` defines `LF_HOME` as "a private, disposable Home,"
yet `installation/server.rs:66-165` registers every non-default `LF_HOME` as a
permanent `KeepAlive` launchd/systemd service (`com.loopflow.server.<hash>`)
and never boots it out.

Chosen for review: **custom homes are never registered with launchd or
systemd.** `ensure()` starts their server as a detached `lf server run` (shared
across macOS and Linux; replacement on macOS pins the listener via
`LOCAL_PEERPID` + `OsProcess` identity check + `SIGTERM` + wait, matching
`linux.rs`'s `SO_PEERCRED`/`pidfd` safety). That server exits when its home
directory is deleted (`home.try_exists() == Ok(false)`, never on transient I/O
error), or after 10 minutes with no client, no held terminal, and no unfinished
Graph or agent. The next launch or `lf server connect` starts it again. The
default home (`~/.lf`) keeps SMAppService and systemd unchanged.

- Cost: a custom home's conversations don't come back after a reboot until
  something runs `lf` there.
- `ensure()` for a custom home boots out that home's old hashed label when
  loaded, so existing registrations retire on first use. A registered server
  that starts and finds its home deleted boots out its own label
  (`XPC_SERVICE_NAME`) before exiting, ending today's crash loops.
- Passed over: an "ephemeral" flag or temp-path detection (a list to extend),
  and tying the server to its launcher's PID (the launcher exits while its
  Sessions continue).

**A holder ends when its home is deleted.** `terminal_holder.rs` today exits
only after 60 s with no child and no reader, leaving 21 orphan holders serving
deleted homes. Chosen for review: a holder periodically checks its `LF_HOME`;
when `home.try_exists()` returns `Ok(false)` (definitive deletion, never on
transient `Err(_)`), it closes its PTYs (children get SIGHUP from the kernel)
and exits. Holders of an existing home are unchanged: they outlive servers and
installs (`docs/dev/server-first.md:242`). `MachineServer::drop` in
`loopflow-test-support` keeps killing only its server; fixtures delete their
home, and the holder follows.

`.lf/qa/hooks.md:69` and `.lf/qa/held-terminals.md:17` lose their "boot that
label out" instructions.

### 9. Graph steps share the cores

Nothing limits concurrent builds: each step's agent runs `cargo` with one job
per core, and `scripts/test.py` caps only its own invocation
(`performance-budgets.json`: 4 jobs, nice 10).

Chosen for review: `RunningGraph::spawn` (`lf/commands/graph.rs:979`), after
`reserve_step(step)` so bursts see themselves, computes
`max(2, cores ÷ running Graph steps on this Machine)` (overridden by new
personal setting `machine.jobs`), exports `CARGO_BUILD_JOBS`,
`NEXTEST_TEST_THREADS`, `MAKEFLAGS=-jN`, and `LF_JOBS` (leaving any preset env
var alone), and lowers the child's priority to `nice 5` in `pre_exec` (never
failing if already nicer). Thirteen Graphs on 16 cores give each build 2 jobs
at nice 5: at most 26 compiler processes at background priority where the brief
saw load 144, leaving `lf server` and Desktop responsive.

- Passed over: a slot semaphore that queues builds. lf does not see which agent
  command is a build, and holding an agent's tool call puts lf in the middle of
  the turn.
- Passed over: deferring suites to CI under load. `gate` already publishes
  first and runs few tests locally.
- Interactive held work sessions are launched by `lf server` → `lf holder`, not
  `RunningGraph::spawn`, so a person's interactive terminal is not throttled.

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
  holds its exclusive lock and its owner (Session, request) is ended. Takers
  re-stat after locking (`fstat` inode equals path inode, else retry) so an
  unlink cannot split waiters onto two inodes.

### 11. Runtime diagnostics without breaking the unversioned `status` probe

`ClientMessage::Status` is sent before `Hello` (unversioned), and `wire::Status`
uses `#[serde(deny_unknown_fields)]` and is mirrored in Swift (`ServerStatus`)
and used by `installation::server::ensure()` across builds during upgrades.
Adding fields directly to `wire::Status` would break older `lf` binaries talking
to a newer server.

Chosen for review: keep `wire::Status` pure readiness. Add a versioned
post-hello operation `server.diagnostics` (bumping protocol revision 10 → 11,
with fixture and `ServerProtocol.swift` / `ServerProtocolTests` updated
together) and CLI command `lf server diagnostics --json` returning
`{threads, store_opens, spawns, held_facts}`.

## Exclusions

- **`processes` (271 MB with indexes), `planning_peer_changes`,
  `git_tracker_changes`**: already marked leaving under LOO-965 and LOO-938;
  nothing has written an `lf` process row since 2026-11-18. Their staged drops
  reclaim the space. Deleting 165k rows here would fire per-row triggers under
  the shared write lock for no lasting gain.
- **Schema size.** 402 triggers make each cold open slow; with pooled
  connections it is paid once per process. Collapsing triggers is its own
  staged migration work.
- **Tracker polling in every Graph agent step** (`watch_owned_work` at 30 s per
  process). Pooled connections make each pass cheap; moving it into the server
  is a separate change.
- Removing dead `repository_plans` rows, in-process Git (`gix`), Linux
  filesystem events for Git facts.
- Stopping the leaked processes on this Machine today. After this ships the 21
  orphan holders are still the old build; they are stopped by PID, by hand.

## What goes

- `observe.rs`: the three per-source refresh threads and their 250 ms sleep;
  background refresh of unheld facts; the `READING`-only branch of `derived`.
- `transcript/watch.rs`'s four separate watcher threads (folded into `observe`'s
  one shared filesystem watcher).
- `read_stream.rs`: `reader_runtime()` per subscription; the 1 s `CHECK` idle
  wake loop; per-subscription permanently held `store: Option<SharedStore>`.
- `settings.rs`: per-key `validate_value` on valid files (the function stays
  for `lf config set` and invalid-file fallback).
- Scattered `Runtime::new()` in command and ops code; `block_on_task`'s runtime
  construction (`ops/task.rs:429`).
- `store/mod.rs` `open_read_only_store`'s validate-then-reopen.
- `installation/server.rs`: plist and unit generation for custom homes, hashed
  labels, legacy `com.loopflow.machine.<hash>` retirement (folded into one
  bootout), and the ignored launchd fixture in `tests/transport_tests.rs:2669`
  with `scripts/test_server_registration.py`'s custom-home cases.
- `maintenance.rs` `start_plan_exchange` as its own thread; `wake.rs`'s own
  watcher thread.
- Tests pinned to the old timing are rewritten to the new rule, not loosened:
  `transcript/watch.rs:113-155` (1100 ms sleep), `observe.rs` `refresh_all`'s
  61 s backdate.

Must survive: external config edits reach a live subscription within 1 s and
reach unheld `load_config()` immediately; process death, deadlines and the
clock invalidate held readings without a store write (`observe.rs:996-1048`,
`:1220-1312`); a server newer than its store refuses (including after data-only
migrations); holders outlive server replacement in a live home; exit codes
remain visible when attaching right after a terminal exits; per-request SQL
receipts in subscription frames; `leaving_guard` in test stores; cross-build
`lf server status` compatibility during upgrades.

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
read through `lf server diagnostics --json`, which returns `threads`,
`store_opens`, `spawns`, and `held_facts` so the same numbers are visible on a
live Machine. The base commit's numbers go in the PR body next to the branch's.

Focused tests that fail if the feature breaks:

- `observe`: an unheld Git fact is not re-run after its interval; a held one
  re-runs after a write under its checkout and not before; an unheld
  `derived()` config read sees a file write made 1 ms earlier immediately.
- `settings`: a file with one unknown key keeps its neighbours and warns once;
  an invalid `stream` or `agent` is still rejected on the fast path; a valid
  file does one `Config` deserialization (counter in `cfg(test)`).
- `loopflow-storage`: two sequential `open_sqlite` calls yield one
  `Connection::open`; per-caller `token_key` is respected across pooled
  checkouts; both DDL and data-only schema changes trigger revalidation; a
  store behind the binary is still refused on a pooled connection.
- `read_stream`: an idle subscription performs zero `step()` calls over 5 s;
  a write to `history.db` does not wake a `Watch::State` subscription.
- `registry`: with another connection holding the write lock for 5 s, a byte
  typed into a held terminal echoes within 200 ms; an attaching view during
  in-flight exit settlement gets `Waiting` and then sees the recorded exit code.
- `installation`: `ensure()` in a custom home registers no launchd job,
  replaces an older detached server cleanly on macOS and Linux, and exits after
  its home is removed; a holder exits after its home is removed and its child
  receives SIGHUP (and does not exit on a live home).
- `graph`: two Graph steps started together on a 16-core fixture each see
  `CARGO_BUILD_JOBS=8` and `nice >= 5`; a preset value is left alone.

After merge, on the live Machine: `sample <server pid> 3` shows no
`serde_yaml_ng`, `sqlite3InitOne` or `posix_spawn` frames with Desktop open and
13 Graphs running, and `lf server diagnostics --json` reports under 20 threads.

## Demo

```sh
lf server diagnostics --json | jq '{threads, store_opens, spawns, held_facts}'
top -pid "$(pgrep -f 'lf server run' | head -1)" -stats cpu,th -l 5
```

With Desktop open on a Task and thirteen Graphs running: CPU reads 0.x, 18
threads, `spawns` unchanged across two calls a minute apart. Then
`LF_HOME=$(mktemp -d) lf task create … --run`, delete the directory, and
`ps` shows that home's server and holder gone within a minute, with
`launchctl list | grep com.loopflow.server.` empty.

## Passes

1. **Measure, then prove the store pool, settings cache, and non-blocking relay.**
   `server.diagnostics` + `lf server diagnostics --json` (with protocol
   revision bump, fixtures, and Swift mirror), `server_idle.py`, base numbers
   recorded. Process-wide SQLite connection pool (§3), one-shot settings
   validation + always-on `derived()` with inline stat check + PATH/transcript
   miss caches (§1 last three bullets, §2), held-terminal thread completely off
   SQLite and transcript files (§6). This first slice proves the riskiest bets
   immediately: pooled connections hold across `TestLedgerGuard` homes,
   per-caller `token_key`s, and schema changes; quiet store opens drop to 0;
   `Hosted::pass` and `validate_value` vanish from `sample`; and typing echoes
   under a 5 s write lock.
2. **Event-driven observation, one watcher, one runtime, quiet subscriptions.**
   Held-only refresh rule and single shared filesystem watcher (§1), one
   bounded runtime + merged background threads (§4), domain-filtered + pushed
   subscription wakeups with 30 s fallback reconcile (§5), scoped minute Git
   plan exchange (§7). Shows idle forks at 0 and threads under 20.
3. **Disposable custom-home lifetimes.** §8 (detached server on macOS + Linux,
   idle/missing-home exit, holder exit on deleted home, bootout of legacy
   hashed jobs, docs in `docs/dev/server.md`, `machines.md`, `.lf/qa/`).
4. **Job budget and bounded files.** §9 (`RunningGraph::spawn` env budget +
   `nice 5` + `machine.jobs` in `docs/config.md`) and §10 (log rotation,
   delivery/log/backup/lock-file pruning).

Each pass deletes what it replaces and leaves the benchmark passing its
then-current budgets.

## Checks

- `scratch/concept-review.html` — pre-implementation concept review written and reconciled into this plan.
- `uv run python scripts/benchmarks/server_idle.py target/release/lf --seconds 60` — not run yet; implement runs it on base and branch.
- `sample 77346 3` on the live server, 2026-11-25 — run at kickoff, results above.

## User docs written before implementation

- `docs/server.md` — `lf server diagnostics --json` example and field reference, plus custom `LF_HOME` detached server and holder lifetimes.
- `docs/config.md` — `machine.jobs` setting and automatic Graph-step build job budget (`CARGO_BUILD_JOBS`, `NEXTEST_TEST_THREADS`, `MAKEFLAGS`, `LF_JOBS`, `nice 5`).
- `docs/troubleshooting.md` — `lf server diagnostics --json`, automatic hourly log rotation and file retention, and custom `LF_HOME` auto-exit.
- `docs/ops.md` — `lf server diagnostics` in the reference table.

Open question surfaced while writing `docs/config.md`:

- **`machine.jobs` vs `Placement` portability and string shorthand:** `machine` in `config/catalog.rs` is currently `Placement` (`PersonalScoped`, `Merge`, `Portable`) and accepts string shorthand (`machine: mini`) as well as `{tasks, session, capacity}`. Should `machine.jobs` be host-local (so a laptop's cap does not forward to a 64-core remote Machine or rental), and should `lf set --global machine.jobs 4` expand an existing `machine: mini` shorthand to `{tasks: mini, session: mini, jobs: 4}` or use a separate host-local key?
