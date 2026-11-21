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

## What the runs established

- The installed `~/.local/bin/lf` matched the live server's release
  (`aeab8b41`). The shell resolved an older `~/.cargo/bin/lf` (`a527444a`),
  and LaunchServices carried Task and Session routing. The Dev launcher
  clears `LF_*` and the Swift-only bundle uses the installed entry.
- The revision-8 peer in `swift/LoopflowTests/ServerClientTests.swift` supplied
  an extra Machine field, multiline Session history and a working Graph
  customization alongside an unsupported action and a missing required Task
  field. The compatible operations kept working. The sealed copy listed Work
  but refused `lf session resume d566a570… --prepare --json` with “This is a
  copy of the Machine from 09:32; make changes in Loopflow or with lf.” The
  live owner's socket is where a second viewer can join the held PTY.
- Live QA used a Swift-only signed bundle under a separate identity because
  LOO-515 was using the shared Dev bundle. Desktop state reported 22 Working
  Tasks and reflected a disposable Task's CLI-created tile and rename in
  place. Both viewers exchanged input on one disposable Session with one
  provider. After close, `show` said “already shown” and `send` said “no
  native surface.” `focus -p session` reattached, and the original provider
  survived. Server instance `5f5bc7b3` and the pre-existing holder PIDs were
  unchanged. The AttributeGraph sheet appeared and its scoped logs had zero
  cycle mentions. The locked screen limited the check to Desktop state,
  captures, input and process inspection. See [QA](qa.html).
- The implementation following [concept review](concept-review.html)
  centralized server-start policy in `MachineLf` and removed the obsolete
  workspace comment. Five focused launcher and server cases passed. The
  older peer saw only `server status`, the private-home fixture counted one
  `install server`, and a stopped-server reply produced recovery guidance
  before the next Dev read connected. A signed probe appeared as
  `Loopflow Dev` on the live server and exited.
- The empty fixture decoded again after supplying the required roadmap
  Tasks field. Adding the repository Task list passed the model test but the
  narrow screenshot still said “Loading work…”. The mounted view changed the
  repository reading key after setup. Both empty shots were removed, leaving
  four loading and unavailable shots in `scripts/screenshots.yaml`.
  `empty-workspaces` still drives the launch smoke check.
- All five headless benchmark fixture cases passed with a CLI-created Task
  and Session on a private Machine. An empty checkout/config sandbox grant
  had exposed the external canary. The corrected fixture denies it. The
  closed Session card's workspace was null and preparation supplied the
  Machine and checkout. The viewer retained its exit message, so reopen
  waits for closure and releases the viewer.
  `uv run python scripts/desktop_performance.py verify-fixture --lf target/debug/lf`
  covers these cases.

- The third [concept review](concept-review.html), at head `6a9783980b`
  against the base, found nothing to change in the model: `MachineLf` is the
  one answer to which `lf` runs and who starts the server, Dev names itself
  to the server, and no copy or world mention is left outside release notes.
  The release-launch benchmark's own SQLite backup
  (`scripts/benchmarks/desktop-performance/launch.py`) stays as a
  measurement input with its own caller.

## Remaining

Nothing here returns to implement. The first item is PR text; the rest need a
display.

- Carry the live QA results and their limits into the PR. Describe voluntary
  cleanup of the old `~/.lf-copy` launchd job after its current demo is done.
  Keep the copy's files and the live Machine's service in place. The code and
  current docs have dropped the sealed-copy and fake-world workflow. The
  WAL-aware snapshot still serves installation and migration.
- A 2026-11-20 rerun found the screen unlocked but the Machine in use (load 14). Its Dev, launched in the background, stays on “Opening repository…” until shown, so the mounted check still needs an idle display.
- At mounted QA, use an unlocked, quiet Machine to launch the normal Dev
  command from a visible Swift-only edit and observe the changed UI on the
  live backlog through a disposable Task update. Compare server and holder
  identities before and after. Use a disposable Session for the attach,
  input and reopen check. QA saw automatic attachments to other Sessions
  during workspace warmup, and two views at different widths resized the
  disposable terminal. Use a live Machine with disposable Sessions only for
  this check. Changes to shared-terminal sizing are outside this
  Task. The one first-entry crash was caught by Breakpad, lost before
  symbolication and did not recur in two attempts under lldb.
- Run the mounted benchmark on the quiet Machine. The previous preflight
  found more than 0.1 core of background CPU and recorded no timings.
