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

October 28, 2026, a `sqlite3 .backup` copy of Jack Heart's live store, one-shot `--json` reads including process start: `lf monitor ps` 0.26 s CPU on this branch's debug build vs 0.39 s on installed release 0.15.0 (per-PID reads; a debug build understates the gain). `lf history --since 7d --limit 50` 2.0 s CPU on the debug build before incremental reading; after it, 0.40 s (0.21 user + 0.19 sys, three runs), decoding 53 of 3,322 inputs in about 15 ms. Its output equals installed 0.15.0's on the same copy.

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
- October 28, 2026 (implement pass): One-shot reads are `Query` (`operation.rs`) beside `Operation`: `TaskChanges`, `TaskDiff` (the draft travels in the query, as `--draft`'s stdin) and `TaskFile`. `Query::read` returns what `lf <argv> --json` prints through the commands' shared `ops::task` functions; `lf_read` serves it. `task_diff_tests::a_query_served_in_process_reads_what_its_lf_command_prints` compares every variant.
- October 28, 2026 (implement pass): A failed in-process watch ends as its process would have exited: `Subscription::ended` / `lf_ended`, after which Swift finishes the part's stream and `WorkModel` reopens the observation.
- October 28, 2026 (implement pass): `WorkPart.operation(for:)` names a `WorkOperation` (Swift's encoding of `Operation`); `WorkWatches` restarts a part only when its operation changed. A Task part without a repository resolves from `/`, which the app's `lf` children ran in.
- October 28, 2026 (implement pass): The remote Task file watch (`lf --machine … task watch-files`) stays an `lf` process: it reads another Machine over SSH. `LocalLineObservation` and `ReaderLaunchConfiguration` remain for it alone. Local Task files use `TaskFileObservation` (FSEvents) and served reads.
- October 28, 2026 (implement pass): `WorkObservationError.configurationChanged` is deleted: an in-app read cannot outlive a replaced `lf`.
- October 28, 2026 (implement pass): Build: `scripts/loopflow-dev.py ffi [--release]` builds `libloopflow_ffi.a` into `target/{debug,release}`; `Package.swift` searches there per configuration (`LoopflowFFI` system library, macOS only), `project.yml` builds it in a pre-build phase. `test_desktop.sh`, `prove_wave_surface_states.sh`, `install.py`, `release-loopflow.py` and the `swift-test` CI job build it first. `macos_inputs.py` sends `loopflow-ffi` and `operation.rs` changes to `swift-test`.
- October 28, 2026 (implement pass): the window-open reads are `Query` variants: `RepositoryIdentity { directory }` (`config repository`), `Waves` (`wave list --all --current`), `MachineId`, `Portfolio`, `PaneLayout { directory, work }` (`desktop layout`), `People { directory }` (`ops people`) and `Graphs { directory }` (`graph list`). Each shares one function with its command (`work_route::repository_identity`, `waves::list`, `machine::local_machine`, `portfolio::read`, `desktop::saved_layout`, `people::people`, `graph_catalog`). `Query::argv` is now the whole command, `--json` included where the command takes it (`desktop layout` always prints JSON). Swift's `ServedQuery` (was `TaskQuery`) keeps the CLI argv without `--cwd` and carries the directory as `cwd` for transports that run `lf`; `RegistryQuery.recording` records served reads too. `RepositoryIdentity` registers a repository on first sight, as the command does, so the app writes the store there as well (see questions).
- October 28, 2026 (implement pass): `viewPlan` is `Query::ViewPlan { directory, expression }` through `desktop::plan_view`, shared with `lf desktop view --plan`; a plan the command exits 3 on (no members, some unavailable) is an error naming the unavailable reasons. The `WorkModel.apply` → `refreshSelectedView` path starts no `lf`. Also served: `GraphProcesses { directory, task, after }` / `GraphProcess { id }` (`graph_inventory::page` / `detail`), `TaskUsage` / `TaskContext` (`usage::sessions` / `context_report`), `WaveDetail { directory, wave }` / `ClosedTasks { directory, wave, after }` (`waves::desktop_status` / `desktop_closed_tasks`), `Machines` (`machine::machines`). Reads with no working-directory dependence dropped Swift's unused `cwd` (`taskHistory`, `taskContext`, `machines`). `WaveDetail` syncs the repository with its tracker in-process, as the command does (see questions). The equivalence test ignores the times a reading stamps (`observed_at` strings, `evidence_age_secs`).

- October 28, 2026 (implement pass): the remaining action reads are `Query` variants, each sharing one function with its command: `History(HistoryWindow)` (`activity::snapshot`), `Roadmap { directory, wave, task, all }` for `desktop plan` (`waves::show_json`; also Task destinations), `PlanningStatus` (`planning::status_json`), `WorkLink` (`open::url_links`), `RepositoryPath` (`work_route::repository_path` + `repository_identity`), `WaveGraph` (`ops::project::wave_graph`), `SessionPage` (`session::read_once`), `Usage` (replaces `TaskUsage`), `Tokens` (`tokens::codebase`), `Retention` (`history_transcript::retention_summary`) and `Doctor` (`doctor::report`). `open --link` and `desktop plan` now resolve from a directory: `context::explain_in`, `work_context(directory, …)` and `show_roadmap(…, directory, …)` replace `open`'s `Cwd` guard and `find_repo_root`/`current_dir` reads. `Roadmap` asks the tracker about each repository it reads, as the command does. `Doctor` served in-app returns the report whatever its checks say; the command still exits 1 on a failed check.
- October 28, 2026 (implement pass, revised in realign): still on `lf` from `RegistryQuery`: writes (`portfolio <arguments>` is `portfolio::change`, a write), the remote `--machine` file watch and `taskLocation` (every caller issues it through `onMachine`, so it is the Machine-selected read `--machine` owns). Outside `RegistryQuery`, `session observe-status` is a status writer that lives only with a terminal.
- October 28, 2026 (realign): `session bind --dry-run` is a read: `changes_state` records a Process only for a bind that assigns, and the preview is `Query::BindPreview { session, task }` through `session::binding_preview` (`human_session::preview_binding`, as the command calls). Swift's `previewSessionBinding` takes no cwd.
- October 28, 2026 (implement pass): `lf history` reads incrementally without a migration. A payload time never follows its row's `observed_at`: the recorder stores a terminal row at `observed_at = ended_at` and a capture fact at the fact's own time, native turn times are the column itself, and the live-store copy has no violation (0 of 8,096 terminal rows, 0 of 39,410 fact rows). Each `PendingInput` carries `latest`, the largest `observed_at` among its captured row, rows naming it in `captured_event` and its native turns' rows. `activity::build_snapshot` decodes by `latest` descending and stops once `limit + 1` entries are strictly newer than the next bound. Covered by `session_events::tests::no_decoded_time_follows_its_inputs_bound` and `activity::tests::reading_settles_once_enough_entries_follow_every_unread_bound`.
- October 28, 2026 (compress pass): Swift's `WorkOperation` and `ServedQuery` encode through one `TaggedFields` (serde's internal tag beside the fields, `null` when absent); `ServedQuery.cwd` is its `directory` field. `RegistryQueryTests.servedReadEncodings` encodes the JSON `operation::tests::desktop_encodings_name_their_reads` decodes. `planning::Status` (remote tracker or Git peers) serves the one-shot, the watch and the text output.

## Remaining work

`lf` watch efficiency:
- `lf history` still selects every input in its window in SQL (3,322 rows on the live copy) before decoding the few it shows; that selection is now most of the 0.40 s. Only worth pushing into SQL if the installed-build profile shows it.
- Measure idle and busy CPU on an installed build that contains this branch. The 0.15.0 baseline above has none of it. Gate owns this if no branch build can be installed headless.

In-app library:
1. Every local `RegistryQuery` read is served in-process: its remaining `run(` calls are writes, `onMachine` and the remote-only `taskLocation`. Only the demo observation remains for "Desktop starts no `lf` processes for reads".
2. The observer marks every mailbox each second in case a filesystem event was lost, so an idle subscription wakes the app about once a second for a revisions query with no reading. Move that check onto the waker thread if it shows in profiles.
3. Demo: open Desktop on this branch and confirm with `lf monitor ps` / Activity Monitor that idle Desktop starts no `lf` and that a new or started Task reaches the Working Tasks list within about a second.

## Delete — do not maintain

Gone: `LocalWorkObservation`, `WorkWatches`' process launch, `RegistryQueryLocal.watchWork`'s process readers, the `lf task diff` / `lf task file` processes behind `TaskFileDocument`, `WorkObservationError.configurationChanged`, and their process-watch tests. `LocalLineObservation` and `ReaderLaunchConfiguration` stay for the remote file watch alone; delete them if that watch moves off `lf --machine`. Each `RegistryQuery.run` read that moves to a `Query` deletes its argv call in the same diff; `TaskQuery` is renamed `ServedQuery`. Also gone: `Query::TaskUsage` (folded into `Usage`), `open`'s process-wide `Cwd` guard and the `session bind --dry-run` process.

## Checks

October 28, 2026, inherited `LF_*` unset. Latest results per check:
- `cargo test -p loopflow --test integration task_diff_tests::` 2/2: 35 queries equal their commands, every `Query` variant included (implement, after incremental history).
- `cargo test -p loopflow --lib -- activity:: session_events:: store::sqlite::sessions:: session_history:: usage::` 41/41; `-- waves:: open:: operation:: doctor:: tokens:: planning::` 69/69; `--test integration work_open_tests::` 13/13, `session_cli_tests::` 19/19.
- `cargo test -p loopflow --lib watch::` 18/18; `operation::` 5/5; `cargo test -p loopflow-ffi` 3/3; `pytest python/tests/test_macos_inputs.py` 15/15.
- `cargo clippy -p loopflow -p loopflow-ffi --all-targets -- -D warnings` and `cargo fmt` clean (implement).
- `scripts/test_desktop.sh` filtered to `RegistryQueryTests|WorkModelTests|NewTaskTests|SessionRenameTests|LaunchJournalTests|TaskHistoryProofTests|WorkDestinationTests|DesktopHeadlessTests|WorkSelectionTests|WorkOpeningCLITests|DesktopNativeSession|DesktopPerformanceTests|TelemetryDashboard|Portfolio` 159/159; earlier filters over `TaskGraphProofTests`, `WorkObservationTransportTests|WorkCLISubscriptionTests|TaskFilesTests|Portfolio*|WorkDestinationTests|NewTaskTests` passed.
- Compress pass: `operation:: planning::` 37/37, `task_diff_tests::` 2/2, `test_desktop.sh --filter 'RegistryQueryTests|WorkObservationTransportTests|WorkCLISubscriptionTests'` 26/26, clippy clean.
- Realign: `task_diff_tests::` 2/2 with `BindPreview` (36 queries), `operation::` 5/5, `test_desktop.sh --filter 'SessionControlsTests|WorkNavigationProofTests|RegistryQueryTests'` 24/24 (display-only proofs skipped), clippy and fmt clean.
- Xcode `LoopflowMac` debug build succeeded before the window-open reads moved; not rerun since.

Gate owns: the Xcode build rerun, installed-build CPU, the integration modules an earlier pass updated (`process_ownership_tests`, `global_commands`, `auth_tests`, `doctor_tests`, `machine_commands`, `machine_credentials`, `planning_foreground_tests`) and the full matrix. Demo owns the idle-Desktop observation.
