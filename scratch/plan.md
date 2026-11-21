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

- Compression checks passed: 51 Swift server, registry and creation tests,
  the repository-move test, four Dev launcher and screenshot tests, and all
  five headless cases in `desktop_performance.py verify-fixture`. Mounted
  checks remain with QA.

## QA (2026-11-20, screen locked)

`scratch/qa.html` has the run. Checked on the live Machine with a separate
QA bundle (the shared Dev was LOO-515's demo): Swift-only build, server
discovery without installing, live backlog, CLI create and rename updating in
place, a second view of a running Session sharing output and input with one
provider, and close then `focus` to reattach. Server instance and holders were unchanged.
The AttributeGraph sheet check passed. The empty fixture decoded again
(`WorkFixture.emptyRoadmap`, test added).

## Next pass (concept review after the build)

`scratch/concept-review.html` has the review. The model holds: Dev is a Swift
window on the installed Machine, with no copy, world or helper of its own.
Four things in the area are still off. Start with the first; it shows soonest
whether the rest fit.

1. **One answer to "does this app start the server".** `controlLfPath`
   chooses among private home, bundle helper and the public entry and returns
   only a path. `ServerLocation.init(bundledHelper:privateHome:)` derives
   `preparesServer` again from different inputs, and
   `GUIProcessEnvironment.enriched` branches a third time. Return the choice
   itself (an enum such as `MachineLf` with `path` and `startsServer`; only
   the public-entry case does not start one). `ServerLocation` takes
   `startsServer` and loses both environment parameters; `enriched` reads the
   same value. Update `ServerClientTests` and `LocalWaveAgentLauncherTests`
   to the new shape. Expected: the peer fixture still sees only
   `server status`; the private-home fixture still sees one `install server`.
2. **The visitor's server-down sentence.** When Dev only looks and the
   server is not ready, say "Loopflow's server isn't running. Open Loopflow,
   or run `lf install server`." followed by `status.error` when there is one.
   Dev still never starts a server; the next read asks status again. One test:
   a not-ready status with `startsServer` false gives that sentence and no
   install call.
3. **Dev names itself to the server.** `ServerEndpoint.unix` sends
   `name: "Loopflow"` for both apps; send the bundle's name so
   `lf server status` lists "Loopflow Dev".
4. **The empty captures show an empty backlog.** `empty-workspaces` supplies
   the empty repository Task list `--repo` waits on. If that needs production
   code reshaped for the fixture, delete the two empty captures instead; do
   not leave a capture named empty that shows loading. Remove the entry from
   `questions.md` either way.

Also fix the comment at `RepositoryWorkspace.swift:13` ("copy or world").

Choices made for review: Dev never starts the server (passed over: running
the installed `lf install server` from Dev, which brings it back but makes Dev
a second owner). Run in Xcode still bundles a branch `lf`; ending that is a
separate outcome.

Remaining after that:
- Mounted visuals and the benchmark need an unlocked, quiet Machine.
- One unreproduced Dev crash on first entry into a live Session pane.
- Document manual cleanup of the legacy `.lf-copy` job in the PR.
