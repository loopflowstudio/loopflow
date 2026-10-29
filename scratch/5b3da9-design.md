# 5b3da9: Desktop live updates without lf processes, and a cheap `lf monitor work`

Jack Heart's direction, October 28, 2026: Desktop reads Loopflow through the Rust library inside the app, with an equivalent `lf` command for every call. `lf monitor work --watch` must also become efficient in its own right. Both parts are in this Task. Full brief: `lf task status 5b3da9`.

## Done when

**`lf monitor work` is efficient on its own**
- An idle store costs a reader under 2% of one core on average over 5 minutes. With several agents active, it stays under a stated budget (proposed: 15% of one core). Measured on an installed build and recorded here.
- Writes that aren't Work state, such as Process rows for read-only commands and build output in checkouts, don't trigger a Work reading.
- A reading touches only what a revision could have changed, not the whole snapshot (about 5.7 MB of Session rows today). Coordinate with LOO-670 on Session paging.
- A reader exits within a few seconds of its parent exiting or crashing, even when a descendant still holds its stdin. No `lf monitor work` with parent PID 1 survives an app quit, crash or relaunch.

**Desktop starts no `lf` processes for reads or live updates**
- The Work observation, task diff, task file and the other `RegistryQuery` reads run in-process through the Rust library. Opening Desktop and leaving it idle starts no `lf` processes and writes no Process rows.
- The replaced Swift process code is deleted in the same diff (`WorkObservation.swift` `--watch` launches, `TaskFileDocument.swift` polling, `RegistryQuery` process reads).
- One WAL watcher in the app serves every view, with the same change semantics as `lf monitor work --watch`. A Task becoming started, a new Task, or a Session change reaches Desktop within about a second, including the sidebar's Working Tasks list.
- The library runs inside the Desktop process: no resident process, `lfd`, socket or hosted service.
- Library reads stay off the main thread and release their pinned read snapshot promptly. No snapshot is held open across a wait, so WAL checkpoints keep running and the WAL doesn't grow while Desktop is open.
- A Rust panic at the library boundary becomes an error in the app, not a crash.

**Every app operation has an `lf` equivalent**
- Each operation is one Rust function returning one DTO, called by both the `lf` command and the app binding.
- A test fails when an app operation has no `lf` equivalent or the two outputs differ.

## Open questions
- The app and CLI `lf` both writing the store, including migrations and app-vs-CLI version skew.
- Reads first; whether writes (task run, ship) move in this Task or later.

## Measurements

October 28, 2026, installed 0.14.19 on Jack Heart's live store, one-shot `--json` reads (process start and store open included):

| Read | CPU per reading | Wall | Watch clock |
|---|---|---|---|
| `lf history --since 7d --limit 50` | 2.6 s user + 1.0 s sys | 3.9 s | 2 s → about every 20 s after `BUSY_SHARE`, ≈18% of a core while idle |
| `lf monitor ps` | 0.2 s user + 0.5 s sys | 1.2 s | 2 s → about every 6 s, ≈12% while idle |

October 28, 2026, installed 0.15.0 (no branch commits), Jack Heart's live store with agents active, cumulative CPU over 301 s: `desktop plan --all` 8.0%, `monitor ps` 7.1%, `desktop sessions` 8.9%, `desktop plans` 7.9%, `planning status` 0.8% of one core; one Desktop's set ≈33%. A Dev app's `desktop plan --all` read 4.3% over the same window.

The `processes` revision moved about 7 times a second with Desktop and agents running. Desktop's own `lf task diff` (about one a second per open Files pane) was the largest writer. Installed-build measurements still wait for a build containing this branch.

October 28, 2026, a `sqlite3 .backup` copy of Jack Heart's live store, one-shot `--json` reads including process start: `lf monitor ps` 0.26 s CPU on this branch's debug build vs 0.39 s on installed release 0.15.0 (per-PID reads; a debug build understates the gain). `lf history --since 7d --limit 50` 2.0 s CPU on the debug build, unchanged by this pass (see Remaining work).

## Decisions

- October 28, 2026 (implement pass): `lf history --watch` re-reads on the 5-minute planning clock, not the 2 s activity clock. It shows committed facts only; the clock just ages the window.
- October 28, 2026 (implement pass): a watch reads its parent's PID before starting any thread, so a parent that exits during startup is still noticed. Covered by `watch::a_watch_ends_with_its_parent_while_stdin_stays_open`.
- October 28, 2026, Jack Heart (comment d946b2bc): the library boundary has Ghostty's shape. The library never hands frames to Swift from its own threads. `watch::Reader::step` is the one reading function: the CLI's `--watch` (`Transport::Stdio`) steps it and streams to stdout; `Transport::Pull` returns a `Subscription` whose waker thread only calls a data-free wakeup when the result may have changed (a store change, a request, the read's clock), and `Subscription::pull` steps the reader on the caller's thread and returns the latest changed frame or nothing, so bursts coalesce. The reading's snapshot is dropped before `pull` returns. `Operation` (`lf/commands/operation.rs`) names each in-app read's argv; History, `monitor ps` and `desktop plan --all` are on it; they take no cwd.
- October 28, 2026, Jack Heart (comment 628f4502), replacing the implement pass's choice to keep read-only Process rows as Work state: only `lf` commands that change state record Process rows, including small edits such as renaming a Task. Reads (status, list, show, diff, file, `--watch` readers) record none, so they never wake Work readers or appear in a Task's work. `Commands::changes_state` (`lf/mod.rs`, with `MonitorCommand` and `HistoryCommand` beside their definitions) classifies every command in exhaustive matches; `lf` marks a read effect-free right after parsing. Unsure cases record (Desktop UI actions, bare `lf pr`, `release notes`, `planning status --watch`, which exchanges planning). `--version` and parser errors still append evidence through early observation. A read still prepares the store it reads (`journal::mark_read` opens, creating or migrating it), so reads work on a fresh Machine; commands that touch no store keep `mark_effect_free_read`. A Graph step records its Process whatever it runs, since the Graph reads the step's result from it: the runner sets `LF_GRAPH_STEP` on its direct child, which clears it so the step's own children are not steps.
- October 28, 2026, Jack Heart (comment d6bdd719): the library boundary is a C ABI passing the existing JSON DTOs, not uniffi. `rust/loopflow-ffi` (`include/loopflow.h`): `lf_subscribe(operation, wakeup, context, error)`, `lf_request`, `lf_pull` / `lf_frame_free`, `lf_unsubscribe` (no wakeup after it returns), `lf_string_free`. Every entry point catches panics; a panicking or failing read arrives as a last `unavailable` frame.

- October 28, 2026 (implement pass): an Operation scoped by its working directory names it, `--cwd <directory>` in its argv (not `--repo`, which requires Git, while `planning status` also serves non-Git Work folders). In-process the same path goes to the command's shared function (`desktop::watch_plans`, `session::list_filter` + `watch_list`, `planning::status_repository` + `watch_status`, `desktop::watch_task_pane`), never the process cwd. `DesktopSessions` parses its flags through `SessionListArgs`' own clap definition, so its defaults are the command's. `PlanningStatus` runs the planning exchange for as long as the subscription lives, as the command does; that makes it the app's first in-process store writer (see open questions).
- October 28, 2026 (implement pass): the equivalence test compares a subscription's first frame with the first frame of `lf <argv> --json --watch`. One-shot `--json` is a different reading for `planning status`: it resolves the planning destination and fails in a Git checkout without one.
- October 28, 2026 (implement pass): every watch in a process shares one observer per store (`watch::follow_store`): one `StoreChanges` thread marks each live mailbox and closes a stdio watch whose parent exited, and it ends with its last watch.
- October 28, 2026 (implement pass): `lf monitor ps` reads only the OS processes its unfinished rows and their receipts name (`OsProcess::read_each`: one kernel query per PID on macOS, one `ps -p` elsewhere), not every process on the Machine.

## Remaining work

`lf` watch efficiency:
- `history` decodes every input in its window to show 50 entries. On a copy of Jack Heart's live store on October 28, 2026 that was 3,353 inputs in 7 days, about 14 events each, with `summary_for_input_matching` (one `read_history` query per input) taking about two thirds of the reading. A sound bound needs an upper limit on an input's entry times before decoding. Those times come from payloads (terminal `ended_at`, fact `observed_at`), not the `observed_at` column, so the bound depends on an invariant nobody has established: a payload time never follows its row's `observed_at`. Next: establish that invariant, or have capture record each input's latest time in a column, then decode inputs newest-first and stop once the 50th entry is newer than every remaining bound.
- Measure idle and busy CPU on an installed build that contains this branch (0.15.0 baseline above has none of it).

In-app library, reads first:
1. One-shot reads still run `lf` processes: `task diff` (`RegistryQuery.swift` file list and patch), `task file`, `task watch-files` (a watch, `TaskFilesFrame`), and the other `RegistryQuery.run` reads. Add an `Operation` for each; a one-shot read is a subscription's first pull until a plain read is needed.
2. The observer marks every mailbox each second in case a filesystem event was lost, so an idle subscription wakes the app about once a second for a revisions query with no reading. Move that check onto the waker thread if it shows in profiles.
3. Build: `scripts/loopflow-dev.py` builds the `loopflow-ffi` staticlib before the Swift build. Add a SwiftPM `systemLibrary` target (module map over `rust/loopflow-ffi/include/loopflow.h`, `link "loopflow_ffi"`) that only `LoopflowMac` depends on (the `Loopflow` library also builds for iOS), the library search path, and the same in `project.yml` and the Swift CI jobs.
4. Swift: `WorkWatches` already reads through a `Launch` returning `lines`, `send` and `cancel`. Have `WorkPart` name an `Operation` (JSON) instead of argv and cwd, and back the watch with `lf_subscribe`: the wakeup schedules `lf_pull` on a serial queue that yields lines, `send` is `lf_request`, and `cancel` is `lf_unsubscribe`. Then delete the process readers below and move `WorkObservationTransportTests`, `DesktopPerformanceTests` and `BuiltLf` onto the library.
5. Each in-app read gets a case in the equivalence test `watch::an_operation_served_in_process_reads_what_its_lf_command_prints` (its match on `Operation` is exhaustive). It covers all seven variants today.

## Delete — do not maintain

Delete each when its reads move to the library. The frame and DTO decoding stays.
- `swift/LoopflowMac/Services/LocalLineObservation.swift` (`LocalLineObservation`, `LocalWorkObservation`).
- The process `launch` path of `WorkWatches` in `swift/Loopflow/Services/WorkObservation.swift`.
- Process-backed reads in `swift/LoopflowMac/Services/RegistryQueryLocal.swift` (`watchFiles`, `watchWork`, the `run` reads), plus `ReaderLaunchConfiguration`, which only readers use.
- `TaskFileDocument.swift` change polling through `lf task diff` / `lf task file` processes.

## Checks

October 28, 2026, inherited `LF_*` unset: `cargo test -p loopflow --lib watch::` 18/18 (the equivalence test now covers all seven Operations); `cargo test -p loopflow-ffi` 2/2; `cargo clippy -p loopflow -p loopflow-ffi --all-targets -D warnings` and `cargo fmt` clean. Gate owns the integration modules the earlier pass updated (`process_ownership_tests`, `global_commands`, `auth_tests`, `doctor_tests`, `machine_commands`, `machine_credentials`, `planning_foreground_tests`) and the full matrix.

Compress, October 28, 2026: `cargo test -p loopflow --lib watch::` 18/18, clippy clean.
