# 9a2459 — Loopflow Dev runs on the live Machine; the old dev environments go

## Outcome

`uv run python scripts/loopflow-dev.py run` builds this checkout's Swift app
and opens it on the Machine that is already running: the installed server,
the installed `lf`, today's Tasks and Sessions. A Swift-only edit builds
Swift and nothing else: no cargo, no install, no migration, no server
restart, no schema or wire comparison, no copy of the store. Loopflow Dev
keeps its bundle id, name and signing identity, so it runs beside Loopflow.

Everything that existed to give Dev some other world is deleted: `lf-world`,
its recipes, the sealed copy of the Machine, `--world`, `--copy`, the
pre-launch wire comparison, and the docs and QA notes built on them.

## The one mechanism: Dev runs the installed `lf`

Today the Dev bundle carries a branch-built `lf` (`_build_tool("lf")`,
`scripts/loopflow-dev.py:933`) and the app refuses any other
(`controlLfPath`, `swift/LoopflowMac/MacLocalWaveAgentLauncher.swift:35`:
"PATH fallback is disabled"). That one fact is why a Swift edit needs cargo,
and why a branch with schema work could not open the store and got a copy.

The change: the Dev bundle carries no `lf`. The launcher passes
`LF_BIN=<the lf on the launching shell's PATH>`, and a bundle without a
helper uses it. The release app is unchanged: it has a helper and uses it.

- `controlLfPath`: a bundle with no helper uses `LF_BIN`; with neither, the
  error names the fix (`lf self install`). The `LF_HOME`-plus-`LF_BIN` case
  the Swift tests use (`BuiltLf.swift`) keeps working.
- `GUIProcessEnvironment.enriched`
  (`swift/LoopflowMac/Services/ProcessEnvironment.swift:37`) drops `LF_BIN`
  and re-sets it only from the bundle; without a helper it keeps the given
  one, so panes and shell tabs get the installed `lf` on PATH.
- Dev's bundle stops shipping `Library/LaunchAgents`
  (`_copy_bundled_tools`); it registers no server.
- The launcher runs `open` with `LF_*` cleared. Seen on this Machine: the
  running Dev (pid 35920, launched by another Task's agent) carries
  `LF_AGENT_CALLER`, `LF_BARE_STEER`, `LF_STEER_SESSION`,
  `LF_CONTEXT_DELIVERY` and `LF_CAPTURE_KEY` from the shell that launched
  it; `open` does pass its environment through, and the comment at
  `loopflow-dev.py:117` says otherwise. `_app_shell_environment()` exists
  and is not used for the launch.

What this was checked against:

- **The server is left alone.** Desktop's first socket lookup runs
  `lf install server` (`ServerClient.swift:169`). `ensure()`
  (`rust/loopflow-execution/src/installation/server.rs:12`) returns the
  running server untouched when its `executable_sha256` is the calling
  `lf`'s. Run here with the installed `lf`: instance
  `5f5bc7b3-59a4-4213-b45e-6490a0dd089d`, sha `aeab8b41…`, same before and
  after.
- **The store is never opened by branch code.** Every CLI child (task
  checkout, `session resume --prepare`, shell tabs, `install adopt`) is the
  installed `lf`. The "installed lf ≥ branch frontier" check before a Dev
  launch has nothing left to guard.
- **Sessions are the real ones.** A Dev pane asks the installed server for
  `session.attach` and joins the held terminal as another view: "every
  attached view of it types, beside any already there"
  (`rust/loopflow/src/server/README.md`, Terminal attachment;
  `transport_tests.rs:1436` covers two views typing). Holders belong to the
  installed `lf`, not to a Dev bundle that the next `run` rewrites. Today a
  holder from a Dev bundle is still running from 02:05 (pid 23818).
- `AppUpdates.adoptIfUpdated` must stay a no-op for Dev (`isInstalled` is
  false for `~/Applications/Loopflow Dev.app`); slice 1 confirms it.

## How far a newer Desktop reads an older server

No pre-launch comparison. What is true is found where it happens:

| Difference | What happens | Where |
| --- | --- | --- |
| Revisions overlap | The link opens at the lower one; features test `welcome.revision` | `ServerLink.swift:83`, `service.rs:411` |
| No revision in common | The link fails with "update the selected installation"; nothing reads | same |
| Server sends a field Desktop doesn't know | Ignored | `Decodable` |
| Desktop needs a field the server doesn't send | That part alone shows "This Loopflow (…) can't read what the server (…) sent. (missing X at path)"; other parts stay current; writes still go | `WorkObservation.swift:462` |
| Desktop sends an operation the server lacks | That request alone answers "this server has no operation `X`; update the selected installation"; the connection stays | `service.rs:596`, `refused_operation` |
| Desktop sends a new field on a known operation | Refused per request where the Rust type denies unknown fields; elsewhere ignored | 79 `deny_unknown_fields` sites |

Two gaps to close, both in Swift:

1. One-shot reads (`ServerClient.read` and its decoding callers) do not carry
   the both-builds sentence; only subscriptions do. They get the same
   sentence.
2. Nothing tests the whole picture. One `ServerPeer` test plays an older
   server: a reading with an extra field decodes, a reading missing a field
   fails its part while another part stays current, a write on the same link
   answers, and an unknown operation's refusal reaches the action that sent
   it. `aReadingThisBuildCannotDecodeNamesBothBuilds` covers the second
   case only.

A branch that changes Rust and Swift together sees its new Swift against the
old server: the changed part says so, the rest works. To see the new server,
install the branch (`lf self install`); that is a person's choice, never the
launcher's. DTOs stay strict; a missing field fails its part and no default
papers over it.

## The copy and the worlds buy nothing that is kept

Tried on the running copy (`~/.lf-copy`, served by another Task's Dev at
revision 8): `lf session resume d566a570… --prepare --json` under the copy's
environment prints "This is a copy of the Machine from 09:32; make changes
in Loopflow or with lf." and starts no holder (the same nine holder pids
before and after). Copied rows name Sessions whose PTYs and holder sockets
live under the real home's runtime directory, so a copy shows lists and
cannot show or type into one Session. A fake world shows stand-in Sessions.
Neither is a way to use the Machine; both go, with no successor.

Made for review: no retained alternative for "branch Rust and Swift against
real data". Passed over: keeping `run --copy`; a second server on the real
store.

## Acceptance at gate

From this branch with a visible Swift-only change (for example a changed
backlog header string), with no `cargo` process started by the launcher:

1. `lf server status` → record `instance`, `executable_sha256`; list
   `pgrep -f 'lf holder run'`.
2. `uv run python scripts/loopflow-dev.py run` → prints "Opening the
   Machine."; Dev's window shows the changed UI over the real backlog.
3. `lf task create '<disposable>'` with the installed `lf` → the Task
   appears in Dev's backlog without relaunch; `lf task edit --title` changes
   it in place. Abandon the Task afterwards.
4. Start one disposable Session, open it in a terminal with `lf resume`, open
   the same Session in Dev: both show the same output, a line typed in Dev
   appears in the terminal view, closing Dev's pane and reopening it finds
   the Session still running. No personal conversation is opened.
5. `lf server status` → same `instance` and sha; the holder pids from step 1
   are all still there.

Headless at gate: `uv run pytest python/tests/test_loopflow_dev.py`,
`scripts/test_desktop.sh` with the server-client filter, the moved Rust
tests by name, `cargo clippy --all-targets -- -D warnings`. Steps 2–4 need a
display and a person's Machine: they belong to qa/demo.

`~/Applications/Loopflow Dev.app` is one path shared by every checkout, and
another Task's Dev is running from it now. The live run checks it is not in
use before installing over it.

## What goes

Rust
- `rust/loopflow/src/world/` (mod.rs, recipes.rs), `src/bin/lf-world.rs`,
  `[[bin]] lf-world` in `rust/loopflow/Cargo.toml`, `pub mod world`.
- Sealed copy: `loopflow_storage::copy::{seal, sealed_at}` and their test;
  `rust/loopflow/src/server/copy.rs`; the copy branch in
  `server/service.rs:184-193` and the per-request refusal at `:733-739`;
  `Operation::changes` (`server/wire.rs:41`); `write_refused` in
  `bin/lf.rs:1516`; `installation::server::stop` (only `world::copy` calls
  it).
- Tests `tests/copy_world.rs`, `tests/world_recipes.rs` and their lines in
  `tests/integration.rs`.

Stays, because others call it: `copy::snapshot` (pre-migration backup,
`lf install` validation copy), `lf server describe`, `executable_sha256` and
`ensure()`, `ready` meaning "answers at a revision this lf speaks",
`MachineServer`, `-- draft:` markers, reader floor, `custom_home_selected`,
install `--preview`.

Already gone (#1755): implicit draft copies, `lf clone`, the Preview capsule
and `WorkFrame.copy`. Leftover: `LF_DB_PATH` in
`scripts/benchmarks/desktop-performance/launch.py:191`, which nothing reads.

Scripts and Python
- `loopflow-dev.py`: `--world`, `--copy`, `COPY_DIR`, `_choose_opening`,
  `_repository_refusal`, `_served_types`, `_wire_difference`,
  `_copy_machine`, the `env.json` merge, `DEV_CONTROL_CONFIG`, and the
  Rust build from `run`/`run-debug`/`install`. `prepare-control-tools` and
  `bundle-control-tools` stay for the Xcode-built app and Swift tests on
  private homes.
- `generate_screenshots.py` world building and the five `world:` shots in
  `screenshots.yaml`; mock and empty shots and the website capture stay.
- `check_attributegraph_cycles.sh --world`; `--sheet` runs on `mock-waves`.
- `disk_cleanup.py` `lf-worlds-` prefix, its doc line and test case.
- `test_loopflow_dev.py` copy and world tests; one test stays and grows:
  `run` launches the Machine with the installed `lf`, clears `LF_*`, and
  starts no cargo.

Swift
- `WaveSurfaceLaunch.world`, `LOOPFLOW_UI_TEST_WORLD`, the vestigial
  `LoopflowDevControl.json` write in `WorkDestinationTests.swift:821`,
  `AppRuntime.isAutomatedTest` (no callers), world wording in comments.

Docs: `swift/README.md` (three-mode table, copy and world sections, build
system rows), `docs/dev/machines.md:599-619,659`, `docs/dev/reference.md:105`,
`docs/dev/codebase.md:64`, `scripts/testing.md:37-39,206-214`,
`scripts/README.md:17`, the desktop-performance README,
`rust/loopflow/tests/README.md:1015`, `.lf/qa/{desktop,auto,hooks,resume,
session-reads,task-conversations}.md`, `.lf/skills/qa.md:4-22`,
`.lf/wave/intelligence/MEMORY.md:405-411`. QA notes that used a world now
say: a private `LF_HOME` with the branch `lf` for changes to `lf`; the live
Machine through Dev for Swift.

Not touched: `~/.lf-copy`, `~/.lf/copies`, any world directory on disk, the
launchd job a copy registered (`~/.lf-copy/home/server.plist` names it; the
PR says how to boot it out by hand), `release/*/NOTES.md`.

## What moves

`world_recipes` held the only test of several CLI behaviors. Each becomes a
test in an existing suite that seeds only its own rows in a `TestRepo` home;
no shared builder replaces `World`.

- `lf desktop done` paging and `lf task list --closed --since`
- `lf task list <query>` (open only, case-insensitive, identifiers)
- `lf task list --subscribe` (a frame only when the list changes)
- `lf task status --json` top-level keys and `next`
- A stopped run reads as `running=false` with its failed step

`world_sessions_resume_live_through_the_stub_provider` tested the stub and is
dropped; the session and task-status subscribe case folds into
`session_list_subscribe.rs`.

`desktop_performance.py run`, `verify-fixture` and `terminal-feedback` need
one Task with one resumable Session. Made for review: they build it in a
private `LF_HOME` by launching the real `lf` against a stand-in `claude`
kept beside the benchmark, no seeded rows. If a resumable Session cannot be
made that way without writing rows directly, those three commands and
`DesktopNativeSessionFixture` go instead, and the PR says so; none runs in
CI. Passed over: keeping `lf-world` for the benchmark alone.

## Exclusions

- `-ui-test-mode` (`empty-workspaces`, `mock-waves`, `live`) and
  `MockWaveFixture` stay: `check_desktop_launch.py` runs on every PR and the
  mock states are loading and error, which no data produces.
- `RepositoryWorkspace.machine` stays: the benchmark launcher still opens
  the app on a private home.
- Private-snapshot performance commands (`snapshot`, `prepare-snapshot`,
  `write-visible`) stay; a person runs them on a copy they made.
- No lenient decoding, no schema negotiation beyond revisions.

## Slices

1. **Dev on the installed `lf`.** `controlLfPath` and `enriched` take
   `LF_BIN` when the bundle has no helper; the launcher stops building and
   bundling `lf`, clears `LF_*`, prints "Opening the Machine."; `--world`
   and `--copy` and everything that chose between them are deleted. Shows
   whether anything in the app assumed a bundled helper (signing, panes'
   PATH, `install adopt`), and whether the acceptance steps 1–3 and 5 hold.
   This is the slice that could prove the plan wrong.
2. **Older server, newer Desktop.** The `ServerPeer` test and the one-shot
   read sentence.
3. **Rust deletion.** Sealed copy, `lf-world`, recipes; the moved tests land
   in the same commit that removes `world_recipes`.
4. **Scripts.** Screenshots, cycle check, disk cleanup, then the benchmark
   fixture (or its removal).
5. **Docs and QA notes**, then the live run of all five acceptance steps.

## Done when

`loopflow-dev.py run` on a Swift-only branch opens Dev on the live backlog
without building Rust, a Task changed from the terminal appears in the open
window, a running Session can be watched and typed into from Dev and a
terminal at once, the server's instance and the holders are the same
afterwards, and `git grep -i 'lf-world\|\.lf-copy'` finds only release
notes.
