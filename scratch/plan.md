# 9a2459 — Loopflow Dev on the live Machine

`uv run python scripts/loopflow-dev.py run` builds this checkout's Swift app
and opens the installed Machine's backlog and Sessions. A Swift-only change
requires no Rust build, CLI or server installation, migration, server restart,
schema comparison or store snapshot. Dev keeps its own bundle identity so it
runs beside Loopflow. Remove the fake worlds and sealed copies, including their
current consumers and docs. Existing Machine data and historical release notes
stay.

Jack Heart, Task steer 88459687-b71e-4a4f-8638-b4dc2cfe1043:

> Two explicit design questions to answer and verify. How permissively can a newer Swift Desktop run against the live Machine through the already-installed server, including additive/unrelated wire differences? Exercise an older response with a newer Desktop, retain valid reads and actions, and localize genuinely unsupported features without upgrading the live schema. If evaluating a separate server or clone, prove what using Sessions means: attach a second interactive view to an already-running disposable Session on its owning Machine, see the same terminal output and type into it, detach/reopen while the original survives. Distinguish that from starting an independent new Session or resuming provider history. Explore routing via the existing owner/installed lf and resume --prepare; copied SQLite records do not copy PTYs or holder sockets. Test any retained alternative end-to-end; drop it if it buys nothing. Do not disturb an existing personal conversation.

## Built and checked

`scripts/loopflow-dev.py` builds a Swift-only Dev bundle, clears inherited
`LF_*` routing and uses the installed `lf` through
`swift/LoopflowMac/MacLocalWaveAgentLauncher.swift`. The launcher has removed
`--world`, `--copy` and the pre-launch wire comparison.

- `uv run pytest -q python/tests/test_loopflow_dev.py` passed three tests,
  including environment cleanup and the Swift-only build path.
- `swift test --package-path swift --filter 'devUsesInstalledLf|privateHomeSelectsExplicitHelper|onlyTheInstalledAppUpdatesItself'` passed three tests.
- A temporary signed Dev bundle passed `codesign --verify --deep --strict`.
  Its only executable under `Contents/MacOS` was Loopflow Dev, and it had no
  launch agents. The temporary location avoided replacing another checkout's
  running Dev. This check did not open a window.
- The workspace identity test now uses a generic private home.
  `swift test --package-path swift --filter aWindowRestoredFromAnotherMachineIsNotShown`
  passed.

The public entry matters on this Machine. The installed app and
`~/.local/bin/lf` had the same hash (`aeab8b41…`). The older
`~/.cargo/bin/lf` on PATH had hash `a527444a…`. LaunchServices inherited
Task and Session routing variables when the previous launcher opened Dev.
An install probe during planning returned the same instance
`5f5bc7b3-59a4-4213-b45e-6490a0dd089d` and executable hash when called via
the installed entry. That result only covered matching executables and left
the first connection dependent on install behavior.

The copy served lists but could not join a held Session. On the copy at
revision 8, `lf session resume d566a570… --prepare --json` returned
“This is a copy of the Machine from 09:32; make changes in Loopflow or with
lf.” and the same nine holder PIDs remained. The live path attaches through
the owning server with `session.attach`. In `rust/loopflow/tests/transport_tests.rs`,
`every_viewer_starts_with_the_snapshot_and_shares_live_output` exercises replay
and bidirectional input on one held terminal. The mounted Dev workflow is
still to be exercised.
Use a private Machine for branch Rust. There is no replacement for the copy.

The one-shot path now carries the responding build from `ServerLink.swift`
through `RegistryQuery.swift` and creation recovery. The previous assumption
that every read could use CLI-printed text failed for Session history. The
server returns a JSON string and the typed decoder needs the quotes. Only
repository paths still use bare text.

- `swift test --package-path swift --filter 'olderPeerKeepsServingCompatibleReadsAndActions|aReadingThisBuildCannotDecodeNamesBothBuilds'` passed. The revision-8 peer refused an unknown write and omitted a required Task page field. The same connection still delivered a Machine identity with an extra field, multiline Session history and a successful Graph customization.
- `swift test --package-path swift --skip-build --filter 'contentRoutedSessionHistory|aRecordFromAnEarlierRevisionReadsItsReceiptAndNeverResends|bulkAnswersAndOversizeRequestsTravelByTicket'` passed all three checks.

`ServerLocation` in `swift/LoopflowMac/Services/ServerClient.swift` now uses
`lf server status` from the first lookup in helper-less Dev. A bundled
helper or explicit private home still prepares its server on first use.

- `swift test --package-path swift --filter olderPeerKeepsServingCompatibleReadsAndActions` passed. The socket fixture in `swift/LoopflowTests/ServerClientTests.swift` checks the CLI arguments on every lookup, so an install call fails the test even if the peer answers correctly.
- `LOOPFLOW_TEST_LF="$HOME/.local/bin/lf" swift test --package-path swift --skip-build --filter failedSetupDoesNotRegisterOnWorkRefresh` passed both private-home recovery cases. The checkout had no built CLI.
- The installed `lf server status` reported a ready socket. The mounted check remains at QA.

`a6e084f8bc` removed the sealed-copy command, server mode, write refusals and
storage seal. The WAL-aware `loopflow-storage/src/copy.rs` snapshot is still
called by `lf/commands/install.rs` and `migration_runner.rs`.

- `cargo test -p loopflow --test integration -- task_list_cli:: session_list_subscribe::`
  passed four tests. The Task reads use a private repository and store with
  Tasks created through the production writer. Open search, Done pagination
  and age filtering, a rename on the list stream, and numbered Task and
  Session requests run through the branch CLI. The numbered-request test
  shares the Task fixture so it also runs on Linux. The provider-lifetime
  fixture in `session_list_subscribe.rs` runs on macOS.
- `cargo test -p loopflow --lib -- a_task_takes_up_its_waves_graph_and_keeps_the_one_it_named mechanical_failure_retains_earlier_step_success`
  passed both tests. A Task at review advertised its next command. A stopped
  Graph reported the failed command after its successful step.
- `cargo test -p loopflow-storage --lib snapshot_keeps_rows_committed_only_to_the_wal`
  passed for the snapshot used by installation and migration.
- `cargo test -p loopflow --test served_reads configuration_edits_use_the_server_without_work_processes`
  passed. The server reported ready and handled configuration reads and
  writes with an unreadable Work store.
- `cargo clippy --all-targets -- -D warnings` passed.

`14242e799c` replaced the benchmark's last recipe in
`scripts/desktop_performance.py`. Git and the branch CLI create the private
checkout, Task and primary Session. A stand-in Claude writes one native turn on
first launch and reads it back on resume. Setup and measurement each own their
foreground server and holder. Ordinary runs use DTO fixtures. Terminal feedback
uses an ANSI file and `/bin/cat`. Both run without preparing a Machine.

- `uv run python scripts/desktop_performance.py verify-fixture --lf target/debug/lf`
  passed all five headless tests, including three reopens, refusal to prepare a
  Session from another Machine's snapshot, retained file selection and the
  external canary. An empty checkout/config grant previously let that canary be
  read.
- Four focused Python checks passed for setup receipts, child teardown, ordinary
  runs and private checkout reads. Headless terminal feedback produced its ANSI
  fixture. Ordinary `run --samples 1` built without a CLI and reached preflight;
  background CPU exceeded 0.1 core, so it recorded no timings.
- The Session card for this closed conversation had a null workspace. Prepared
  Session data carried the Machine and checkout. In a private PTY the stand-in
  replied to typed input and quit. Its attached viewer kept displaying the exit
  message, so the reopen benchmark waits for the Session to close and releases
  the viewer. Mounted snapshot timing and
  Ghostty terminal interaction remain for QA.
- Removed the launch benchmark's retired `LF_DB_PATH` override.
  `uv run pytest -q python/tests/test_desktop_launch.py` passed all three
  headless tests.

The screenshot manifest now has six fixture shots (loading, unavailable and
empty at 900 and 1440 pixels). Website capture still has its three live views.
Removed the recipe builder, its Rust binary and the remaining launch and
cleanup inputs. A tracked-file sweep for `lf-world` and `.lf-copy` outside
scratch found only historical release notes.

The sheet check needs the Portfolio window: mock launches open a repository
window by default. `--sheet` requests Portfolio, selects the infrastructure
Wave and presents the create sheet from the mounted detail pane. The script
requires the sheet's appearance marker as well as a clean cycle log.

- Four focused Python tests passed for the fixture capture environments,
  website manifest, signing settings and temporary-directory cleanup.
- `swift test --package-path swift --filter MockWaveFixtureTests` passed
  five tests. `bash -n scripts/check_attributegraph_cycles.sh` passed.
- `cargo clippy --all-targets -- -D warnings` passed after removing the builder.

## Mounted capture checks at QA

- Run `uv run python scripts/generate_screenshots.py` and inspect the six
  fixture images at both widths.
- Run `scripts/check_attributegraph_cycles.sh --sheet` and verify the create
  sheet opens over the infrastructure Wave without AttributeGraph cycles.

## Live acceptance at QA

The shared `~/Applications/Loopflow Dev.app` is replaced by each run. At the
packaging check another Task's Dev (PID 35920) was using its bundled CLI to
serve `~/.lf-copy`. Check that the existing Dev is idle before taking over
the shared path. Any cleanup of that legacy job is a manual step documented
in the PR. Leave data and running personal Sessions alone.

1. Use `~/.local/bin/lf server status` to record `instance` and
   `executable_sha256`. Record existing holder PIDs.
2. Make a temporary visible Swift edit and run
   `uv run python scripts/loopflow-dev.py run`. Check the changed UI over
   the real backlog and that the launcher starts no Cargo build.
3. Create a disposable Task with the installed CLI. Check the row appears,
   then run `lf task update <id> --title 'Renamed QA Task'` and watch it
   change in place without reopening Dev. Abandon the disposable Task.
4. Open an already-running disposable Session with `lf resume <session>`
   and open that same Session in Dev. Check the same output and that a line
   typed in Dev appears in the terminal view. Close and reopen the Dev pane
   and confirm the original Session is still running.
5. Check that server instance, executable hash and original holder PIDs
   match step 1.

These checks require a mounted display and disposable Work on the live
Machine. Headless packaging, server-peer tests and CLI probes cannot close
them. Use `.lf/qa/desktop.md` for the walkthrough.
