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
the owning server with `session.attach`. The Rust transport test already
covers input from two views. The mounted Dev workflow is still to be exercised.
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

The sealed-copy command, server mode, write refusals and storage seal are
gone. The install and migration paths still use the WAL-aware snapshot.
The branch's `lf-world --help` lists only build, list and help.

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

## Remaining implementation

1. **Next — remove world consumers.** Delete the `world:` entries in
   `scripts/screenshots.yaml` and the builder in `generate_screenshots.py`.
   Keep empty and mock screenshots and website capture. Remove the cycle
   check's `--world` and use `mock-waves` for `--sheet`. Remove
   `WaveSurfaceLaunch.world`, `LOOPFLOW_UI_TEST_WORLD`, the unused
   `AppRuntime.isAutomatedTest`, world wording and the `lf-worlds-` cleanup
   prefix and test. Keep mock loading/error states.
   The benchmark's `run`, `verify-fixture` and `terminal-feedback` still
   delegate to `lf-world` in `scripts/desktop_performance.py`. Try a private
   `LF_HOME` with the real branch CLI and a stand-in Claude. Verify the
   resumable Session through the benchmark's actual entry points. If it
   requires seeding rows, remove those commands and
   `DesktopNativeSessionFixture` with their docs. Remove the inert
   `LF_DB_PATH` launch setting. Preserve independent private-snapshot
   commands, `RepositoryWorkspace.machine` and Xcode/private-machine helper
   build commands. Delete `rust/loopflow/src/world/` and the `lf-world`
   binary registration with their last consumer.
2. **Sweep remaining references and exercise changed automation.** The main
   Dev and QA docs were updated in `1ba82f64d0`. Check Rust, Swift, scripts,
   tests, CI and current docs again after consumer deletion. Check
   the moved Rust fixtures by name, focused Python automation and Desktop
   tests, and run `cargo clippy --all-targets -- -D warnings` at gate.
   `git grep -i 'lf-world\|\.lf-copy'` outside scratch should find only
   historical release notes. Implicit draft copies, `lf clone`, the Preview
   capsule and `WorkFrame.copy` were already retired in #1755.
   Use the Task's history anchors for the sweep, from explicit live-ledger
   opt-in (44657d3f5c) and removal of the production DB escape hatch (#964)
   through worlds (#1612, #1614, #1617, #1619, #1622), Preview (#1620, #1645)
   and sealed copies (#1996).

## Live acceptance at QA

The shared `~/Applications/Loopflow Dev.app` is replaced by each run. At the
packaging check another Task's Dev (PID 35920) was using its bundled CLI to
serve `~/.lf-copy`. Check that the existing Dev is idle before taking over
the shared path. Any cleanup of that legacy job is a manual step documented
in the PR. Leave data and running personal Sessions alone.

1. Use the installed `lf server status` to record `instance` and
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
