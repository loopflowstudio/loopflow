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

The `processes` revision moved about 7 times a second with Desktop and agents running. Desktop's own `lf task diff` (about one a second per open Files pane) was the largest writer. Branch measurements wait for an installed build containing this branch; #1755 removed `lf clone` data copies, so there is no isolated copy of the live store to read.

## Decisions

- October 28, 2026 (implement pass): `lf history --watch` re-reads on the 5-minute planning clock, not the 2 s activity clock. It shows committed facts only; the clock just ages the window.
- October 28, 2026 (implement pass): a watch reads its parent's PID before starting any thread, so a parent that exits during startup is still noticed. Covered by `watch::a_watch_ends_with_its_parent_while_stdin_stays_open`.
- October 28, 2026, Jack Heart (comment d946b2bc): the library boundary has Ghostty's shape. The library never hands frames to Swift from its own threads. `watch::Reader::step` is the one reading function: the CLI's `--watch` (`Transport::Stdio`) steps it and streams to stdout; `Transport::Pull` returns a `Subscription` whose waker thread only calls a data-free wakeup when the result may have changed (a store change, a request, the read's clock), and `Subscription::pull` steps the reader on the caller's thread and returns the latest changed frame or nothing, so bursts coalesce. The reading's snapshot is dropped before `pull` returns. `Operation` (`lf/commands/operation.rs`) names each in-app read's argv; History, `monitor ps` and `desktop plan --all` are on it; they take no cwd.
- October 28, 2026, Jack Heart (comment 628f4502), replacing the implement pass's choice to keep read-only Process rows as Work state: only `lf` commands that change state record Process rows, including small edits such as renaming a Task. Reads (status, list, show, diff, file, `--watch` readers) record none, so they never wake Work readers or appear in a Task's work. `Commands::changes_state` (`lf/mod.rs`, with `MonitorCommand` and `HistoryCommand` beside their definitions) classifies every command in exhaustive matches; `lf` marks a read effect-free right after parsing. Unsure cases record (Desktop UI actions, bare `lf pr`, `release notes`, `planning status --watch`, which exchanges planning). `--version` and parser errors still append evidence through early observation. A read still prepares the store it reads (`journal::mark_read` opens, creating or migrating it), so reads work on a fresh Machine; commands that touch no store keep `mark_effect_free_read`. A Graph step records its Process whatever it runs, since the Graph reads the step's result from it: the runner sets `LF_GRAPH_STEP` on its direct child, which clears it so the step's own children are not steps.
- October 28, 2026, Jack Heart (comment d6bdd719): the library boundary is a C ABI passing the existing JSON DTOs, not uniffi. `rust/loopflow-ffi` (`include/loopflow.h`): `lf_subscribe(operation, wakeup, context, error)`, `lf_request`, `lf_pull` / `lf_frame_free`, `lf_unsubscribe` (no wakeup after it returns), `lf_string_free`. Every entry point catches panics; a panicking or failing read arrives as a last `unavailable` frame.

## Remaining work

`lf` watch efficiency:
- Readings still rebuild the whole result. Start with `history` (whole 7-day window decoded, then cut to 50) and `monitor ps` (samples every OS process).
- Measure idle and busy CPU on an installed build that contains this branch (0.15.0 baseline above has none of it).

In-app library, reads first:
1. `Operation` for the reads that take a repository cwd: `desktop sessions`, `desktop plans`, `planning status`, `desktop task`. Their commands read the process cwd; pass the repository explicitly instead. Then task diff, task file and the other `RegistryQuery` reads (one-shot: first frame, or a plain `read`).
2. One `StoreChanges` per app serving every subscription; each subscription starts its own observer today.
3. `rust/loopflow-ffi` exists (wakeup/pull). A one-shot read is the first pull of a subscription until a plain read is needed. The observer marks the mailbox every second in case a filesystem event was lost, so an idle subscription wakes the app about once a second for a revisions query with no reading; move that check onto the waker thread if it shows in profiles.
4. Build: `scripts/loopflow-dev.py` builds the staticlib before the Swift build. Add a SwiftPM system-library target and module map, and update `project.yml` and CI.
5. Swift: replace the process readers and delete them (see below).
6. Equivalence test `watch::an_operation_served_in_process_reads_what_its_lf_command_prints` covers History and DesktopPlan. It now covers every variant (`Operation` is exhaustive and the test matches it), including `monitor ps`, which no longer lists its own Process. Extend the match as Operations are added.

## Delete — do not maintain

Delete each when its reads move to the library. The frame and DTO decoding stays.
- `swift/LoopflowMac/Services/LocalLineObservation.swift` (`LocalLineObservation`, `LocalWorkObservation`).
- The process `launch` path of `WorkWatches` in `swift/Loopflow/Services/WorkObservation.swift`.
- Process-backed reads in `swift/LoopflowMac/Services/RegistryQueryLocal.swift`, plus `ReaderLaunchConfiguration` if only readers use it.
- `TaskFileDocument.swift` change polling through `lf task diff` / `lf task file` processes.

## Checks

`cargo test -p loopflow-ffi`: 2/2. `cargo test -p loopflow --lib watch::`: 18/18 after compress moved the pull deadline into the mailbox (needs every inherited `LF_*` unset; the equivalence test fails otherwise). Integration, with inherited `LF_*` unset: `process_ownership_tests`, `global_commands`, `auth_tests`, `doctor_tests`, `machine_commands`, `machine_credentials`, `planning_foreground_tests` pass after updating expectations to the read rule; the integration modules the earlier killed run never reached pass (163/165: `session_cli_tests::available_lists_only_conversations_ready_for_input` missed under load and passed alone; `session_steers::public_context_capability…` fails on this Machine's broken Homebrew node, `libllhttp` missing). Clippy `--all-targets -D warnings` and `cargo fmt --check` clean. Gate owns the full matrix. After merging main (#1755 removed `lf clone`; `lf self install reader-catalog` classified as a read): `cargo check -p loopflow --all-targets` clean.
