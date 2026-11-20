# 343ad5: Loopflow Dev opens a read-only snapshot of the Machine

## Outcome

`uv run python scripts/loopflow-dev.py run` on any branch opens Loopflow Dev on
the Machine's real Waves, Tasks and Sessions as of that moment, served by the
branch's own server from a read-only copy of the store. The installed server,
its store and every running Session are not touched: the Dev build never
connects to them.

Done when:

- On a branch that changes a wire DTO or adds a migration, `loopflow-dev.py run`
  shows the real repository window with today's Tasks, not empty lists.
- While Loopflow Dev runs, the installed `lf server status` reports the same
  `instance` and its pid is unchanged, and `~/.lf/loopflow.db` has no row the
  Dev build wrote.
- A change made in Loopflow Dev (run a Task, steer, create) answers with one
  sentence saying this is a snapshot, and starts no provider.
- `loopflow-dev.py run --world <dir>` opens a fake world and shows only that
  world's windows.
- swift/README.md and scripts/testing.md describe this one way.

## Demo

On a branch with a wire change (LOO-903's `owned_by_you` is the case that
failed): `uv run python scripts/loopflow-dev.py run`. Loopflow Dev opens the
loopflow repository with the real backlog. In another terminal `lf server
status` still shows the installed server's instance. Click Run on a Task in
Dev: "This is a snapshot of the Machine from 14:02; make changes in Loopflow or
with lf."

## What was found

**The executable check is not what emptied the lists.** `lf server status` from
a non-installed build compares the server's executable hash with its own and
reports "Loopflow server uses another executable"
(`rust/loopflow-execution/src/machine/client.rs:83`). Reproduced November 20
with the Dev bundle's lf (3500440e9) against the installed server
(21d541a87). That comparison only feeds `installation::server::ensure`, which
uses it to decide whether to re-register after a promotion. `Connection::open`
and Swift's `ServerLink` negotiate by protocol revision alone, and `lf install
server` from a Dev build on the default home execs the installed lf
(`delegate_selected`). The installed server's log shows it served the Dev app
`config.theme` at 08:57:53Z, so the link was up.

**The lists were empty because the branch changed the wire.** LOO-903 added a
required `owned_by_you` to `local_task`, `wave_detail` and `roadmap_snapshot`
(`git diff 21d541a87 3500440e9 -- tests/fixtures/dto`). Wire types carry no
defaults, both protocol revisions are 8, so the hello passed and every snapshot
failed to decode. A Dev app on the installed server works only for branches
that leave every DTO and operation alone, and nothing tells the two cases
apart before the window is empty. That route is closed.

**A branch server on the real home is a second live Machine.** The server
starts `wake` (messages idle conversations through their real inboxes) and
`recovery` (starts Task conversations it believes died, in held terminals) at
boot (`rust/loopflow/src/server/service.rs:186`, `recovery.rs`, `wake.rs`). On
the default home it would also fight for the socket and the SMAppService
registration. Closed by the November 4 constraint.

**A snapshot a branch build can read takes two seconds.** Probe, November 20:
`sqlite3 -readonly ~/.lf/loopflow.db ".backup …"` copied the 688 MB store in
2.0 s. With `LF_HOME` at the copy, the Dev bundle's lf listed the real Tasks
(`task list`), Sessions (`session list`) and `wave status infrastructure`.
`store::copy::snapshot` (`rust/loopflow-storage/src/copy.rs`) already does this
copy with an APFS clone for `lf install`.

**A read-only file is the whole containment.** After `chmod 444` on the copy,
the same three reads still worked, and `lf task create` failed with "sqlite
error: attempt to write a readonly database" before filing anything. The
private home also minted its own `provider-token.key`, so the copied service
tokens don't decrypt there.

**The fake-world workaround failed for three separate reasons.**
- Restored windows: `WindowGroup(for: RepositoryWorkspace.self)` restores the
  Dev bundle's saved windows whatever `LF_HOME` it runs with
  (`swift/LoopflowMac/LoopflowApp.swift:93`), so the world got the real
  repository's window.
- Path length: the world's log has `Desktop API unavailable … error="Socket
  path exceeds 103 bytes"` for `home/run/desktop/com.loopflow.mac.dev.sock`
  under the long scratchpad path.
- The installed Loopflow.app (pid 36218, started 01:08:52) was running with the
  world's `LF_HOME` and the demo Session's `LF_AGENT_CALLER`, without `LF_BIN`.
  It rewrote the world's `server.plist` to `/Applications/Loopflow.app/…/lf`
  and its `install server` failed with "Bootstrap failed: 5". It did not come
  from `loopflow-dev.py` (that always sets `LF_BIN`). What launched it is not
  established; see `scratch/questions.md`.

## Design

A **snapshot world** is a world directory (`env.json`, `home/`, empty provider
homes) whose `home/loopflow.db` and `home/history.db` are read-only copies of
the Machine's. `lf-world snapshot <dir>` makes one: stop the server that last
served `<dir>`, `copy::snapshot` both databases from the default home, open the
store once so a branch migration applies (a private home migrates itself),
checkpoint, `chmod 0444`. The directory is replaced on each call.

The store file's mode is the single source of truth. Three behaviors derive
from it, none from a flag or marker:

- **The server acts only on a store it can write.** A server whose store opens
  read-only serves reads and subscriptions and does not start `wake`,
  `recovery`, `maintenance` or remote control.
- **Commands fail at the store.** The server's command dispatch turns that
  failure into one sentence written in Rust and carried on the wire: "This is a
  snapshot of the Machine from HH:MM; make changes in Loopflow or with lf." The
  time is the database file's modification time.
- **No provider can run.** The world's `CLAUDE_CONFIG_DIR` and `CODEX_HOME` are
  empty, as in a fake world, and a launch cannot record its Session.

`loopflow-dev.py run` and `run-debug` take `--world <dir>`. Without it they run
`lf-world snapshot ~/.lf-view` and open that. `~/.lf-view` is fixed and short:
one launchd job (`com.loopflow.server.<digest of the home>`) is reused, not one
per run, and `~/.lf-view/home/run/desktop/com.loopflow.mac.dev.sock` is 62
bytes. Both pass `-ApplePersistenceIgnoreState YES`, so the window opened is
the `--repo` one and nothing is restored from another home.

`lf server status` on the default home execs the installed lf the way `lf
install server` does, so a branch build reports the installed server's real
readiness.

Choices made for review:

- **`run` always opens a snapshot; the Dev app no longer joins the real
  Machine.** Passed over: keeping today's live path for branches that leave the
  wire alone. It gives live terminals and real changes from Dev, and it is the
  path that silently empties on a wire change and lets branch code write the
  real store.
- **Read-only by file mode, not a server flag.** Passed over: a writable copy
  with `lf server run --observe`. A flag covers the server but not the lf
  children Desktop starts.
- **The snapshot is as of launch.** Running `run` again refreshes it. Passed
  over: a branch server reading the live store read-only, which fails on any
  branch with a migration and holds read transactions on the real WAL.

## Acceptance check at gate

One Rust integration test, `snapshot_world`: seed a home with a Task and a
Session whose attachment looks live, with a stand-in provider on `PATH`; run
`lf-world snapshot`; start `lf server run` on the snapshot.

- A served Task read returns the seeded Task.
- A `task.run` command returns the snapshot sentence.
- After 2 recovery intervals the stand-in provider was never executed.
- The source `loopflow.db` is byte-identical before and after, and the
  snapshot's is unchanged after the command.

`cargo test -p loopflow --test integration snapshot_world` passes. The launch
arguments get a new Python test beside `scripts/loopflow-dev.py`; none exists
today.

## Surfaces

| Surface | Change |
| --- | --- |
| `lf-world snapshot <dir>` | New. Prints the same `env.json` shape as `build`. |
| `lf server run` | Serves a read-only store without its three writers. |
| Server command dispatch | One sentence for a read-only store. |
| `lf server status` | Delegates on the default home. |
| `scripts/loopflow-dev.py` | `run`/`run-debug` default to the snapshot; `--world` on both. |
| Loopflow Dev | Shows the server's sentence through the existing notice path. No DTO change. |
| swift/README.md, scripts/testing.md, docs/dev/machines.md, docs/dev/reference.md | One way, described once. |

## Absent and error states

- No `~/.lf/loopflow.db`: `lf-world snapshot` says there is no Machine to copy
  and names `lf-world build`.
- A branch migration fails on the copy: the snapshot command fails with the
  migration error; nothing launches.
- Held terminals: a snapshot home has no holders, so a Session pane shows the
  existing "no terminal" state. Terminal UI work uses a fake world.
- Session rows show the liveness the OS reports now; everything else is as of
  the snapshot time.

## Operational boundary

- Snapshot plus migration under 5 s for a 700 MB store on APFS; `.backup` took
  2.0 s without the clone.
- The copy reads the live store under one read transaction and never writes it.
- `~/.lf-view` is private to the account (0700): it holds the full store.

## Exclusions

- Live terminals of real Sessions inside Loopflow Dev.
- Changing real data from a Dev build.
- Cleaning up the `com.loopflow.server.<digest>` launchd jobs that test homes
  of other worktrees left running (19 on November 20).
- Why the Dev app showed "The Machine is away" for a decode failure.

## What goes

- `scripts/loopflow-dev.py`: the `LF_HOME` pass-through in `_app_environment`,
  and the "ordinary app commands never migrate the shared database" hint in
  `_check_repository_opening`.
- swift/README.md "Build system": the paragraphs on the bundled helper
  forwarding Main-Machine commands and on `run` checking the shared schema.
- docs/dev/machines.md: "Loopflow never copies main data into it".
- docs/dev/reference.md Fake world row: "Nothing is shared with a real Machine"
  gains the snapshot.

Stays: `status()`'s executable comparison (it decides re-registration after a
promotion), `delegate_selected`, fake-world recipes, the installed app's window
restoration.

## Slices

1. **A branch server serves a read-only snapshot and starts nothing.** The one
   that could prove the plan wrong. `lf-world snapshot`, the server's
   writable-store check, and the `snapshot_world` test without the sentence.
   It shows whether served reads and subscriptions (which stamp observations)
   work on a store they cannot write, whether WAL mode holds on a 0444 file
   under a long-lived server, and that recovery and a pane launch start no
   provider. If served reads need writes, the fallback is a writable copy whose
   actors stay off; decide then, with the failing read in hand.
2. **`loopflow-dev.py run` opens it.** Default snapshot at `~/.lf-view`,
   `--world` on both commands, `-ApplePersistenceIgnoreState YES`. Check by
   launching Dev on this branch and on a fake world; if SwiftUI ignores the
   AppKit default, `.restorationBehavior(.disabled)` when `--repo` is passed.
   Remove the `LF_HOME` pass-through.
3. **The sentence, and `lf server status` delegation.** Command dispatch maps
   the read-only failure; the acceptance test gains its assertion.
4. **Docs.** swift/README.md, scripts/testing.md, machines.md, reference.md.
