# 343ad5: Loopflow Dev on real data, on the Machine when it can and on a copy when it can't

## Outcome

`uv run python scripts/loopflow-dev.py run` on any branch opens Loopflow Dev on
the Machine's real Waves, Tasks and Sessions, and says in one line which of two
things it opened:

- **The Machine**, when the branch's lf opens the store and speaks the wire the
  installed server speaks. This is today's path: live terminals, real changes.
- **A copy of the Machine**, when either differs, or with `--copy`. The branch's
  own server serves a read-only copy of the store taken at launch. The
  installed server, its store and every running Session are not touched.

`--world <dir>` opens a fake world, as now. Those are the three things a Dev
build can look at: the Machine, a copy of it, a fake world.

| | Real data | Changes work | Live terminals | Branch may change wire or schema |
| --- | --- | --- | --- | --- |
| The Machine | yes | yes | yes | no |
| A copy | yes, as of launch | no, refused with one sentence | no | yes |
| A fake world | no | yes | stand-in providers | yes |

Done when:

- On a branch that changes a wire DTO or adds a migration, `loopflow-dev.py run`
  prints which types differ, opens a copy, and the repository window shows
  today's Tasks, not empty lists.
- On a branch that leaves both alone, `run` opens the Machine as it does today.
- While Loopflow Dev runs on a copy, the installed `lf server status` reports
  the same `instance` and pid, `~/.lf/loopflow.db` has no row the Dev build
  wrote, no real Session process was signalled, and no file changed in any real
  worktree.
- A change made on a copy (run a Task, steer, create, start a Session) answers
  with one sentence and runs no command code.
- `loopflow-dev.py run --world <dir>` shows only that world's windows.
- `lf server status` from a branch build says the installed server is ready.
- swift/README.md and scripts/testing.md describe this once.

## Demo

On a branch with a wire change (LOO-903's `owned_by_you` is the case that
failed): `uv run python scripts/loopflow-dev.py run` prints

```
This branch changes what the server sends (GraphCatalogEntry, GraphSubject, RoadmapTask, Write).
Opening a copy of the Machine from 14:02; changes are refused there.
```

Loopflow Dev opens the loopflow repository with the real backlog. In another
terminal `lf server status` still shows the installed server's instance. Click
Run on a Task in Dev: "This is a copy of the Machine from 14:02; make changes
in Loopflow or with lf."

On `main`, the same command prints "Opening the Machine" and a Session's
terminal is live.

## What was found

**The executable check is not what emptied the lists.** `lf server status` from
a non-installed build compares the server's executable hash with its own and
reports "Loopflow server uses another executable"
(`rust/loopflow-execution/src/machine/client.rs:83`). Reproduced November 20
with the Dev bundle's lf (3500440e9) against the installed server
(21d541a87). That comparison only feeds `installation::server::ensure`, which
uses it to decide whether to re-register after a promotion. `Connection::open`
and Swift's `ServerLink` negotiate by protocol revision alone. The installed
server's log shows it served the Dev app `config.theme` at 08:57:53Z, so the
link was up.

**The lists were empty because the branch changed the wire.** LOO-903 added a
required `owned_by_you` to `local_task`, `wave_detail` and `roadmap_snapshot`
(`git diff 21d541a87 3500440e9 -- tests/fixtures/dto`). Wire types carry no
defaults, both protocol revisions are 8, so the hello passed and every read
failed to decode. Revision 8 names the framing and negotiation, not the shape
of what is sent. The shape is already one document: `lf server describe
--json`, checked in as `tests/fixtures/dto/server_schema.json`. It needs no
store and no running server, so two builds can be compared before anything
opens.

**A branch server on the real home is a second live Machine.** The server
starts `wake` (messages idle conversations through their real inboxes) and
`recovery` (starts Task conversations it believes died, in held terminals) at
boot (`rust/loopflow/src/server/service.rs:186`, `recovery.rs`, `wake.rs`). On
the default home it would also fight for the socket and the SMAppService
registration. Closed by the constraint that a running Session never breaks
because a Dev build started.

**A copy a branch build can read takes two seconds.** Probe, November 20:
`sqlite3 -readonly ~/.lf/loopflow.db ".backup …"` copied the 688 MB store in
2.0 s; `history.db` is 98 MB. With `LF_HOME` at the copy, the Dev bundle's lf
listed the real Tasks (`task list`), Sessions (`session list`) and `wave status
infrastructure`. `store::copy::snapshot` (`rust/loopflow-storage/src/copy.rs`)
already does this copy with an APFS clone for `lf install`.

**A read-only file stops store writes, and only those.** After `chmod 444` on
the copy, the same three reads still worked, and `lf task create` failed with
"sqlite error: attempt to write a readonly database" before filing anything
locally. Whether it reached Linear first was not checked. The private home
minted its own `provider-token.key`, so the copied service tokens don't decrypt
there. A copied store still names real things outside itself: live pids, real
worktrees, tracker ids, PR numbers.

**Copies of the Machine were built once and deleted.** `lf clone` (#1549,
October 14) made a writable copy: it scrubbed credentials, routes, pids,
endpoints and attachment tokens and ran everything in a macOS sandbox, about
1,000 lines with its tests. #1755 (October 29) deleted it with the draft-era
store machinery; one first use had taken 265 s against an 8.8 GB store. Its
rule was "a branch with no new migration runs its own code directly on
`~/.lf`; a branch with one runs against a fresh private `LF_HOME`", and
docs/dev/machines.md says "Loopflow never copies main data into it". Two
things have changed: the copy now takes 2 s, and a fresh private home has no
real data, which is the gap this Task reports. This design keeps the first
half of that rule and replaces the second. It does not bring back the scrub or
the sandbox: the copy is never writable.

**The fake-world workaround failed for three separate reasons.**
- Restored windows: `WindowGroup(for: RepositoryWorkspace.self)` restores the
  bundle's saved windows whatever `LF_HOME` it runs with
  (`swift/LoopflowMac/LoopflowApp.swift:93`). `RepositoryWorkspace` is `{id,
  path}` and does not say which Machine it belongs to, so the world got the
  real repository's window.
- Path length: the world's log has `Desktop API unavailable … error="Socket
  path exceeds 103 bytes"` for `home/run/desktop/com.loopflow.mac.dev.sock`
  under the long scratchpad path. Nothing said so before the window opened.
- The installed Loopflow.app (pid 36218, started 01:08:52) was running with the
  world's `LF_HOME` and the demo Session's `LF_AGENT_CALLER`, without `LF_BIN`.
  It rewrote the world's `server.plist` to `/Applications/Loopflow.app/…/lf`
  and its `install server` failed with "Bootstrap failed: 5". It did not come
  from `loopflow-dev.py` (that always sets `LF_BIN`). What launched it is not
  established; see `scratch/questions.md`.

**Desktop already has "this window cannot change the Machine right now".**
`PendingChanges.Link.away` disables change controls and shows a standing
notice, "The Machine is away; showing what was last read". The Dev app showed
that sentence when its reads failed to decode, though the link was up: one
undecodable frame failed the whole observation and every reopening failed on
the same frame (slice 4).

**`welcome` and `status` reject unknown fields** (`#[serde(deny_unknown_fields)]`
in `rust/loopflow-execution/src/machine/wire.rs`). A field added to either
breaks every older Rust client at connect, including the lf inside running
Sessions after an install. Neither is changed here.

## Design

### The launcher chooses, and says which

`loopflow-dev.py run` and `run-debug` share one choice, made after the build:

1. `--world <dir>`: that world.
2. `--copy`: a copy.
3. Otherwise the Machine, when both hold:
   - the bundled lf opens the store (`lf config repository --json`, the check
     `_check_repository_opening` already runs), and
   - the bundled lf's `server describe --json` equals the installed lf's. The
     installed lf is the public `lf` entry run with every `LF_*` cleared.
4. Otherwise a copy. The launcher prints why: the store's refusal, or the
   names under `$defs` whose definitions differ.

The comparison is the whole document. A difference that would have decoded
fine opens a copy when the Machine would have worked; that errs toward the
safe side and is not tuned further.

`LF_HOME` reaches the app only from this choice. The pass-through of an
inherited `LF_HOME` in `_app_environment` goes: a shell that carries a world's
environment no longer decides what Dev opens.

### A copy of the Machine

A copy is a world directory (`env.json`, `home/`, empty provider homes, and the
same `bin/` stand-ins a built world has for `gh`, `launchctl` and the
providers) whose `home/loopflow.db` and `home/history.db` are read-only copies
of the Machine's. `lf-world copy <dir>` makes one: stop the server that last
served `<dir>`, `copy::snapshot` both databases and copy `config.yaml` (the
person's name and which repositories the Machine shows) from the Machine its
shell is on (`LF_HOME`, else `~/.lf`), open the store once so a branch
migration applies (a private home migrates itself), then `copy::seal`:
checkpoint, stamp the file's modification time, `chmod 0444`. The stamp is
needed because an APFS clone keeps the source's time, the Machine's last
checkpoint. `<dir>` is replaced when it is absent or holds `env.json`; anything
else is refused, since the command deletes it. The launcher clears `LF_*`,
builds `lf-world` beside `lf` when it needs a copy and always uses `~/.lf-copy`:
one launchd job (`com.loopflow.server.<digest of the home>`) is reused, and
`~/.lf-copy/home/run/desktop/com.loopflow.mac.dev.sock` is 62 bytes.

The word is "copy" everywhere a person reads it: the command, the directory,
the sentence, the docs. "Snapshot" already means a subscription's frame
(`roadmap_snapshot`) and `copy::snapshot`.

The store's mode is the one fact; nothing is flagged or marked. What follows
from it:

- **The server acts only on a store it can write.** A server whose store opens
  read-only serves reads and subscriptions and does not start `wake`,
  `recovery`, `maintenance`, remote control, the held-terminal registry or the
  mobile listener. `session.attach` answers the existing "no running
  terminal".
- **That server admits no change.** A `Write`, a Session start, continue or
  boundary, an upload or mobile setup (`Operation::changes`) is answered at
  dispatch, before any command code runs, with one sentence written in Rust:
  "This is a copy of the Machine from HH:MM; make changes in Loopflow or with
  lf." The time is the database file's modification time. The error code is
  `invalid`, which no client retries. Refusing at admission, not mapping the SQLite failure
  afterwards, is what keeps a command from signalling a real pid, touching a
  real worktree or calling the tracker before its store write fails.
- **An lf that Desktop starts beside the server** (a pane launch) meets the
  read-only store and the world's stand-ins: no provider login, a `gh` that
  refuses writes, a token key that decrypts nothing.

The first is the server's own conduct, the second covers everything that goes
through it, the third is the backstop for what doesn't. The acceptance check
measures the outside effects directly, since the mode alone does not promise
them.

Desktop shows the refusal through the notice path every refused write uses. No
DTO changes.

### What a copy shows

- Planning, Tasks, Waves, Sessions and history as of the copy's time.
- Session rows show the liveness the OS reports now, since the copied rows name
  real pids. Nothing in a copy signals them.
- No held terminals: the copy's home has no holders, so a Session pane shows
  the existing "no terminal" state. Terminal work uses the Machine or a fake
  world.
- Tracker and PR refreshes do not run; rows show what the store last recorded.
  Seen November 20 on the real copy: all 254 Task rows read tracker state
  `available` with their stored `synced_at`, PR rows carry their stored
  publication, and the copy's log held no warning. The window itself was not
  seen (the screen capture was black); whether a row turns **Last known** once
  its stored sync time ages is not established.

### Windows belong to their Machine

A window value names the Machine it was opened on: the home directory
(`LF_HOME`, else `~/.lf`), since a copy carries the Machine's own id. A
restored window whose Machine is not the one this launch is on takes the
launch path: it opens the launch's repository on this Machine and closes.
This replaces a launcher flag: the installed app opened on a world's home, as
on November 20, is covered too. Saved windows from before the change carry no
Machine and take that path once. Two windows of one plan on two Machines are
different windows, so opening the copy's never raises a stale one.

### A world says when its directory is too long

`lf-world build` and `lf-world copy` print one line on stderr when
`<dir>/home/run/desktop/com.loopflow.mac.dev.sock` exceeds the socket limit,
naming the length and the limit. They do not refuse: every fixture that builds
a world under macOS's temporary directory is past the limit (118 bytes in
`copy_world`) and none of them opens Desktop.

### `lf server status` says what is true

`ready` means the server answers at a revision this build speaks. A different
executable is not an error: status reports the server's executable hash, and
`installation::server::ensure`, the one caller that needs "the same executable"
to decide re-registration, compares it. A branch build reports the installed
server ready without exec'ing another lf.

### A read that cannot be decoded is not "the Machine is away"

A reading whose body this build can't decode fails its part alone; the other
parts stay current and the link stays up. The part's Notice names both builds,
from `welcome.build` and the app's own, and where decoding stopped: "The plan
couldn't be read: This Loopflow (0.16.0+3500440e9) can't read what the server
(0.16.0+21d541a87) sent. (missing owned_by_you at body.tasks.0)". The
launcher's comparison keeps Dev from getting here; the installed app meets it
whenever the app and server builds part.

## Choices made for review

- **`run` opens the Machine when the branch speaks its wire, a copy
  otherwise.** Passed over: always a copy. It is one behavior, but it takes
  live terminals and real changes away from Swift-only branches, where today's
  path works, and it drops the half of #1755's rule that held. Also passed
  over: always the Machine, failing on a difference; that leaves wire-changing
  branches with fake data only.
- **The copy is read-only.** Passed over: a writable, disposable copy, so that
  dragging and creating can be tried on real data. Everything a copied row
  points at is real, so it needs the scrub and sandbox #1755 deleted.
  Interactions on a wire-changing branch are tried in a fake world.
- **Read-only by file mode, refused at admission.** Passed over: `lf server
  run --observe` on a writable copy; a flag covers the server but not the lf
  children Desktop starts. Also passed over: letting commands run and wording
  the SQLite failure; effects outside the store come first.
- **The copy is as of launch.** `run` again refreshes it. Passed over: a branch
  server reading the live store read-only, which fails on any branch with a
  migration and holds read transactions on the real WAL.
- **Restored windows are fixed in the app, not by a launcher flag.**
- **`status` changes its meaning of `ready`; nothing delegates.**

## Outside this work

- A copy's window does not say it is a copy until a change is refused. A
  standing label needs the sentence on the wire; `welcome` and `status` reject
  unknown fields, so it takes a new read that older servers lack, plus a
  second reason beside `away` for `PendingChanges.acceptsChanges` to be false.
- What launched the installed app on the world's home.
- The `com.loopflow.server.<digest>` launchd jobs that test homes of other
  worktrees left running (19 on November 20). A copy's own job stays loaded
  after Dev quits and is replaced by the next `lf-world copy`.
- Live terminals of real Sessions on a wire-changing branch.

## Acceptance check at gate

One Rust integration test, `copy_world`: seed a home through the CLI with a
Task conversation whose stand-in provider is running and a second checked out
and never started; run `lf-world copy`; start `lf server run` on the copy.

- A served Task read returns the seeded Task. *(in the test)*
- `task.run`, a Session start and a Session stop each return the sentence.
  *(in the test)*
- The Machine has one Task conversation whose provider is alive and one
  prepared with no server to start it. The copy's log says "ready on a copy of
  the Machine" and holds no failed write, the stand-in provider never ran and
  the live provider is still running. *(in the test)*
- A pane launch through the copy's lf starts no provider. *(in the test)*
- The source `loopflow.db` is byte-identical before and after, the copy's is
  unchanged after the commands, and the seeded checkout has no new or changed
  file. *(in the test)*

`python/tests/test_loopflow_dev.py` covers the choice (same documents and an
opening store: the Machine; a differing `$defs` entry: a copy, naming it; a
refusing store: a copy; `--copy`; `--world`) and the environment each passes.
*(built)* A Swift test covers a restored window from another Machine
*(built)* and a decode failure's notice. *(built)*

Checks, November 20 at 0715825870:

- `cargo test -p loopflow --test integration copy_world`: 2 passed. Against an
  `lf` built with `wake`, `maintenance` and `recovery` started on a copy, the
  first test failed at the log assertion: that server logged "store retention
  failed", "terminal exit not recorded" and "recovery pass failed" within
  80 ms of ready. CI runs the suite.
- `uv run pytest python/tests/test_loopflow_dev.py`: 14 passed.
- Slice 3: `cargo test -p loopflow --lib world::tests::` 1 passed;
  `copy_world` 2 passed again after the shared setup;
  `scripts/test_desktop.sh --filter 'aWindowRestoredFromAnotherMachineIsNotShown|samePlanReusesItsRetainedWorkspace|WorkOpeningCLITests|WorkspaceRetentionTests'`
  21 passed, before the launch view gained its cancellation branch and log
  line; the Dev bundle built with them.
- Not run: clippy on the branch, the full Swift suites, `world_recipes` and
  `two_people` after the shared setup. Gate and CI.

## Surfaces

| Surface | Change |
| --- | --- |
| `scripts/loopflow-dev.py run`, `run-debug` | Choose the Machine or a copy and print which; `--copy` and `--world` on both. |
| `lf-world copy <dir>` | New. Prints the same `env.json` shape as `build`. Both say on stderr when `<dir>` is too long for Desktop's socket. |
| `lf server run` | Serves a read-only store without its writers and admits no change. |
| `lf server status` | `ready` no longer depends on the executable. |
| Loopflow, Loopflow Dev | A restored window from another Machine opens the launch's repository instead; a decode failure has its own sentence. |
| swift/README.md, scripts/testing.md, docs/dev/machines.md, docs/dev/reference.md | The three things Dev opens, described once. |

## Absent and error states

- No `~/.lf/loopflow.db`: `lf-world copy` says there is no Machine to copy and
  names `lf-world build`.
- No installed lf to compare with: a copy, saying so.
- A branch migration fails on the copy: the command fails with the migration
  error; nothing launches.
- The Machine's store has a migration the branch doesn't know: neither the
  Machine nor a copy opens; the launcher prints the store's refusal and names
  `lf sync main`.

## Operational boundary

- Copy plus migration under 5 s for a 700 MB store on APFS; `.backup` took
  2.0 s without the clone.
- The copy reads the live store under one read transaction and never writes it.
- `~/.lf-copy` is private to the account (0700): it holds the full store.

## What went

All of these are gone from the branch (`grep` on November 20 finds none
outside `scratch/`).

- `scripts/loopflow-dev.py`: the inherited-`LF_HOME` pass-through in
  `_app_environment`; the "ordinary app commands never migrate the shared
  database" hint in `_check_repository_opening`, replaced by the choice's own
  line.
- `client.rs`: the "uses another executable" error in `status()`.
- swift/README.md: the opening paragraph and "Build system" paragraphs on which
  lf runs where are rewritten as the three cases; "Main-Machine commands
  forward to installed lf" goes unless the code is found to do it.
- docs/dev/machines.md: "Loopflow never copies main data into it".
- docs/dev/reference.md Fake world row: "Nothing is shared with a real Machine"
  gains the copy.

Stays: today's path onto the Machine, `delegate_selected` for `lf install
server`, fake-world recipes, `welcome` and `status` as they are.

## Slices

1. **Built.** A branch server serves a read-only copy, starts nothing and
   admits no change: `lf-world copy` (`world::copy`), `copy::seal` and
   `copy::sealed_at` in storage, `server/copy.rs`, `Operation::changes`,
   `installation::server::stop`, and `copy_world`.

   What it showed, on the real store on November 20: `lf-world copy` of the
   688 MB store took 2.8 s. The branch's `lf server run` on the copy answered
   a `plan.list` subscription with the real Tasks, and `task.run`,
   `task.steer`, `session.continue` and `request.upload` with the sentence.
   SQLite created `-shm` and an empty `-wal` beside each 0444 file and read
   through them. `lf -b default` and `lf resume <session>` through the copy's
   lf both stopped at "attempt to write a readonly database" before any
   provider ran.

   The read-only store is also what stops recovery. With `wake`, `recovery`
   and `maintenance` started on a sealed copy that held a conversation due
   for recovery, no provider ran: `continuing` writes before it starts one.
   What that server does is log failed writes from boot on (retention,
   statistics, "recovery pass failed", and "terminal exit not recorded" every
   2 s). So `copy_world` reads the copy's log; a provider check alone passes
   either way. On a writable copy the same server started the provider 2 s
   after boot.

   A world's `World::checkout` prepares no Task conversation, unlike
   `lf checkout`; `one-task` has none. The test seeds through the CLI.
2. **Built.** `_choose_opening` in `scripts/loopflow-dev.py`: `--world`, then
   `--copy`, then the Machine when the bundled lf opens the repository and
   both `lf server describe --json` documents are equal, else a copy with the
   reason printed. `lf-world` is built beside `lf` only when a copy is made.
   The inherited `LF_HOME` no longer reaches the app.
   `python/tests/test_loopflow_dev.py` covers each branch of the choice and
   the environment it launches with.

   What it showed, November 20, this branch against installed 21d541a87:
   - The two documents are equal, so `run` prints "Opening the Machine."
     (the choice was run; Dev was not launched on the Machine). Against
     LOO-903's document the names are GraphCatalogEntry, GraphSubject,
     RoadmapTask and Write.
   - `run --copy` printed "Opening a copy of the Machine from 01:53; changes
     are refused there." Dev's window for `/Users/jack/src/loopflow` listed
     the 19 working Tasks (`lf desktop ls`). Its server logged "ready on a
     copy of the Machine" and nothing else. The installed server kept
     instance 033984e9 throughout.
   - A second `run --copy` booted out the loaded job
     `com.loopflow.server.530be3a272dbf20a` (pid 57908) and the same label
     came back as pid 63838: `installation::server::stop` works against a
     loaded job.
   - `run --world` on a `one-task` world showed one window, the world's.
     The restored-window case of slice 3 did not appear here, since the
     bundle's saved window was the real repository's and the world got its
     own.
   - Reading Dev's own socket from a shell needs
     `LF_DESKTOP=com.loopflow.mac.dev`; without it `lf desktop` looks for the
     installed app's socket in the copy's home.

   Not done: the `lf sync main` line is printed only when the Machine refused
   the bundled lf and the copy then failed; the launcher cannot tell a store
   ahead of the branch from any other failed copy.
3. **Built.** `RepositoryWorkspace` carries `machine`, and `LoopflowApp`
   mounts a window only when it was opened on this launch's Machine;
   `world::prepare` is the one setup `World::create` and `world::copy` share,
   and `WorldEnv::desktop_socket_notice` is the line `lf-world` prints.

   What it showed:
   - The refusal the plan had could not stand. With it, both `copy_world`
     tests failed at fixture setup: "…/T/.tmpnT4riv/machine/home/run/desktop/
     com.loopflow.mac.dev.sock is 118 bytes and a socket path holds 103".
     `world_recipes`, `two_people`, the Swift fixtures and four scripts build
     worlds the same way. The notice replaced it.
   - A launch window that loses its opening to a newer request for the same
     path used to show the cancellation as an error with Retry. Two stale
     restored windows would do that to each other, so the launch view now
     closes on cancellation.
   - Seen November 20 with Dev at 8808224489. macOS restores a window only
     after an exit that was not a quit: after quitting, the next launch's
     window value was `nil`. After `kill -9` of Dev on a `one-task` world at
     `/tmp/w343`, a launch on `~/.lf-copy` restored a window whose value was
     the world's (`machine: "/tmp/w343/home"`). It took the launch path,
     opened `/Users/jack/src/loopflow` on the copy and closed: eight seconds
     later AppKit had two windows, "Loopflow Dev" not visible and "loopflow"
     visible, and `lf desktop ls` listed one, the copy's. Read from temporary
     log lines; the screen could not be captured. So the fallback flag
     `-ApplePersistenceIgnoreState YES` is not used.
   - `CGWindowListCopyWindowInfo` kept listing the closed launch window as on
     screen, on every launch including one with no restoration. AppKit's
     `isVisible` is what was read.
   - `lf-world build one-task` in a directory 18 bytes too long printed "Loopflow Dev
     opens this world without `lf desktop`: …/home/run/desktop/
     com.loopflow.mac.dev.sock is 121 bytes and a socket path holds 103. Use
     a shorter directory." and built the world.
   - `LoopflowState.selectedRepoPath` is one file per account, not per
     Machine. An app launched on another home without `--repo` asks that
     Machine for the last repository the account opened anywhere. The
     launcher always passes `--repo`, so Dev does not meet it.
4. **Built.** `client::status()` reports `ready` whenever the server answers
   at a revision this build speaks and carries the server's
   `executable_sha256`; `installation::server::ensure` compares that hash with
   its own executable and registers again when they differ. A reading whose
   body does not decode fails its part alone, with `ServerLink.unreadable`
   and where decoding stopped as the reason (`WorkFrame.unread`,
   `LineSubscription.unreadable`). Desktop's client version carries its source
   revision, so the sentence tells a Dev build from the installed one.

   What it showed:
   - Why a decode failure read "The Machine is away": one undecodable frame
     failed the whole observation (`WorkSubscriptions.receive`), and
     `WorkModel.keepWorkCurrent` calls `pendingChanges.linkLost()` on every
     error. Each reopened observation failed on the same frame, so the link
     never returned and went away after ten seconds; `NoticeCenter.read`
     holds part failures while the link is not connected, so nothing else was
     said. `keepWorkCurrent` is unchanged: a server that is not running also
     arrives there as a plain error and should read as away.
   - Seen November 20: `target/debug/lf server status` (sha 619fe20a5a5e)
     against the installed server (executable bde305c4900a, 0.16.0+21d541a87)
     printed `ready: true`, `error: null` and the installed lf's `instance`,
     033984e9. Run with the branch's debug `lf`, not the Dev bundle's copy.
   - Other readers of `status()`: `lf server status`, `ServerLocation.path`
     in Desktop (reads `socket`, `ready`, `error`), and test fixtures that
     wait on `ready`. None wanted the executable compared.
   - A line that is not a reading at all, or a transport that does not know
     its server (`lf … --subscribe` over a pipe), still fails the observation.

   Checks: `cargo test -p loopflow --test served_reads configuration_` 2
   passed (its fixture now waits on `status().ready` from the test binary,
   another executable than the server's);
   `scripts/test_desktop.sh --filter 'aReadingThisBuildCannotDecodeNamesBothBuilds|invalidFrames|scopeChange|serverEndingASubscriptionFailsItAlone'`
   4 passed. Not run: clippy, the iOS build, `lf install server` replacing a
   server left by another executable (`transport_tests`), and the Notice in a
   Dev window against a server of another wire. Gate and CI.
5. **Docs: read against the code after slice 4, no difference found.**
   Written ahead of the code; each slice corrected its part. Read, not run:
   `docs/server.md`, `docs/troubleshooting.md` and the server README give
   `ready` the meaning `client::status()` has; swift/README.md's Notice is the
   string `aReadingThisBuildCannotDecodeNamesBothBuilds` asserts.
   - `swift/README.md`: the opening section (the three things Dev opens, the
     copy's sentence, windows per Machine, the socket-length notice), the
     decode Notice after "Last known", "Build system" and its command table.
   - `docs/dev/machines.md`: `lf-world copy` and the read-only server under
     "One main Machine"; one boundary contract.
   - `docs/dev/reference.md`, `docs/dev/codebase.md`: the Fake world row and
     `lf-world` line.
   - `docs/server.md`, `docs/troubleshooting.md`,
     `rust/loopflow/src/server/README.md`: what `ready` means; the decode
     Notice in place of "Build mismatch".
   - `scripts/testing.md`: `run` and `copy_world`.
   - `.lf/qa/desktop.md` and `.lf/qa/held-terminals.md` no longer quote
     "uses another executable" (slice 4). Neither scenario was run again.

## Remaining

No slice remains. Gate and CI run what the slices list under "Not run".

The sentence for an undecodable reading has one carrier,
`LineSubscription.unreadable`: `ServerClient.subscribe` takes it from the link
it connects. The pipe transport and two test transports pass `{ nil }`.

QA, November 20 (`scratch/qa.html`): an `lf` that meets the sealed store
prints the copy's sentence (`server::copy::write_refused`), and the decode
Notice writes an array position as a number. Checks:

- `cargo test -p loopflow --test integration copy_world`: 2 passed with the
  pane's sentence asserted.
- The Swift edit built into the Dev bundle; its test
  (`aReadingThisBuildCannotDecodeNamesBothBuilds`) and clippy were not run.
  Gate and CI.

Concept review after the build, November 20 (`scratch/concept-review.html`):
the three things Dev opens, the sealed store as the one fact, `ready` apart
from "the selected installation", and windows named by home directory each
match one concept in the code. Nothing changes in the plan. Two pairings are
partial and stay as built:

- A change on a copy has two keepers: the server refuses at dispatch; an `lf`
  on the copy's home runs until the sealed store stops it. One keeper would
  need the CLI to know which commands change things, which nothing lists.
- The launcher formats the copy's time itself; the sentence's is in Rust.
