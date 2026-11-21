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

## Implemented and checked

- `scripts/loopflow-dev.py` builds and signs a Swift-only Dev bundle, clears
  inherited `LF_*` routing and opens with its own app identity.
  `MacLocalWaveAgentLauncher.swift` uses the account's public
  `~/.local/bin/lf`. On this Machine it matched the installed app's CLI
  (`aeab8b41…`), while PATH resolved an older `~/.cargo/bin/lf`
  (`a527444a…`). LaunchServices had inherited Task and Session routing.
  `ServerClient.swift` uses `lf server status` from Dev's first lookup.
  Preparing a server is still available for a bundled helper or private home.
  The installed entry reported a ready socket. The temporary Dev bundle passed
  `codesign --verify --deep --strict` and contained only the Swift executable
  in `Contents/MacOS` and no launch agents. No window was opened.
- `ServerLink.swift` and `RegistryQuery.swift` carry the responding build and
  keep typed JSON intact, including the quotes on multiline Session history.
  The revision-8 test peer returned a Machine identity with an extra field,
  Session history and a successful Graph customization even after refusing an
  unknown write and omitting a required Task-page field. The peer fixture
  also checks that all Dev socket lookups use status. The affected Swift
  launcher, update, workspace, private-home and receipt checks passed, as did
  `uv run pytest -q python/tests/test_loopflow_dev.py`.
- The sealed copy listed Work but could not join a held Session.
  `lf session resume d566a570… --prepare --json` on the revision-8 copy said
  “This is a copy of the Machine from 09:32; make changes in Loopflow or with
  lf.” The same nine holder PIDs remained. The owning server's
  `session.attach` path has replay and shared input coverage in
  `every_viewer_starts_with_the_snapshot_and_shares_live_output`
  (`rust/loopflow/tests/transport_tests.rs`). Dev uses the live owner.
  An explicit private Machine remains available for branch Rust.
- `a6e084f8bc` removed the sealed-copy command, server mode, write refusals and
  storage seal. The WAL-aware snapshot still serves installation and
  migration. Focused Rust tests passed for that snapshot, CLI backlog and
  Session lists, Task and Session numbers, Task Graph progress and served
  configuration edits with an unreadable Work store.
- `14242e799c` replaced the performance recipe with a Task and Session made
  by the branch CLI on a private Machine. All five cases passed in
  `uv run python scripts/desktop_performance.py verify-fixture --lf target/debug/lf`.
  An empty checkout/config grant had exposed the external canary. The
  corrected fixture denies it. A closed Session card had a null workspace,
  while preparation returned the Machine and checkout. The stand-in replied
  to typed input in a private PTY. Its viewer still displayed its exit
  message, so reopen waits for closure and releases the viewer. The ordinary
  benchmark built without a CLI but recorded no timings because preflight
  measured over 0.1 core of background CPU. Headless terminal feedback
  generated its ANSI fixture. The three launch benchmark Python tests passed
  after the retired `LF_DB_PATH` override was removed.
- `aa9cad208a` removed the five recipe-backed captures, the `lf-world`
  builder and its remaining consumers. `scripts/screenshots.yaml` keeps six
  fixture shots (loading, unavailable and empty at 900 and 1440 pixels) and
  three live website views. The tracked `lf-world` and `.lf-copy` references
  outside scratch are historical release notes. Four focused Python tests
  for capture routing, signing and cleanup, five `MockWaveFixtureTests` and
  `cargo clippy --all-targets -- -D warnings` passed.
- The mock path returns before the live registry selection finishes.
  `scripts/check_attributegraph_cycles.sh --sheet` requests the Portfolio
  window and `WavesView.swift` raises the create sheet from the mounted
  infrastructure detail. A sheet appearance marker is required, and unified
  logs are scoped to the test bundle. This covers presentation over the
  fixture's loading detail. The former populated registry exercise still
  needs a live window. Shell syntax was checked. Sheet appearance and cycle
  output await mounted QA.

## Next for QA

Follow `.lf/qa/desktop.md`. The shared `~/Applications/Loopflow Dev.app` is
replaced by each run. At packaging, another Task's Dev (PID 35920) was using
its bundled CLI to serve `~/.lf-copy`, so packaging used a temporary bundle.
Check the shared app is idle before replacement. Leave existing Machine
data and personal Sessions alone. Document manual legacy-job cleanup in
the PR.

1. Record live server instance and executable hash with
   `~/.local/bin/lf server status` and record existing holder PIDs.
2. Make a temporary visible Swift edit and run
   `uv run python scripts/loopflow-dev.py run`. Verify the edited UI over
   the live backlog and confirm the launcher starts no Cargo build.
3. Create and rename a disposable Task with the installed CLI. Check the
   row appears and updates in place without relaunching Dev, then abandon
   the Task.
4. Open an already-running disposable Session with `lf resume <session>`
   and open the same Session in Dev. Verify shared output and a line typed
   in Dev from the original view. Close and reopen the Dev pane and confirm
   the original Session survives. Compare server identity and original
   holder PIDs with step 1.
5. Render and inspect the six fixture images with
   `uv run python scripts/generate_screenshots.py`. Run
   `scripts/check_attributegraph_cycles.sh --sheet` and verify the create
   sheet appears over the infrastructure Wave without cycles. In live Dev,
   select a populated Wave and verify that its details render.
6. Check mounted snapshot timing and Ghostty terminal feedback with the
   benchmark instructions in `scripts/benchmarks/desktop-performance/README.md`.

Mounted checks require a display. Record any that cannot be observed.
