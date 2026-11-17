# LOO-926: route typed operations, not command lines

## Problem

`Operation = Server | Read | Write` (`rust/loopflow/src/server/wire.rs`) is the
one request type, and `Read::read` / `Write::run` are the one implementation of
each operation. But only the server's socket carries the value. Every hop
between `lf` processes carries argv instead, so routing is written once per hop
and each hop re-parses with Clap. The Task brief lists the duplicates. Mapping
the code at `470da475a1` showed it is worse than the brief:

- **Three SSH launch sequences, not two.**
  - `lf/commands/ssh.rs::run_with_io` (:71–386).
  - `machine/task.rs::run_remote` (:238–329, engine Task writes).
  - `Write::prepare_session` (`write.rs:1113`), which goes through
    `work_route::Forward::run` into `ssh::run_with_io` with argv from
    `Write::cli()`.
  - "the destination said" appears in `run_remote` and `prepare_session`.
- **The CLI never calls `Write::run`.** Its only caller is the server
  (`server/service.rs:1317`). CLI writes go through `mutation::run`
  (`mutation.rs:51`, gate `accepts` :9) or `dispatch` arms. They reach the
  same `ops::*` / `engine::*` leaves, except where they diverge:
  - config set/unset: the CLI calls `setting::declare`, served calls
    `engine::config::set`;
  - session writes: the CLI calls `session::mutate`, served calls
    `session::renamed/bound/moved/status_recorded`.
- **Forwarding is decided in four places, not three.**
  - `--machine` in `bin/lf.rs:1703`.
  - `work_route::forward` (`work_route.rs:258`).
  - `engine::task::remote/remote_on` (`loopflow-execution/src/engine/task.rs:308,321`).
  - `transport::delegate_current` (`transport.rs:247`), which sends the
    local server `ServerOperation::SessionStart { argv: std::env::args() }`.
- **The engine cannot send an `Operation`.** `Read`/`Write` live in the
  application crate, `engine::task` lives in `loopflow-execution`. So the
  engine spells argv instead: `spell_run` :347, `spell_task` :368. The same
  spelling feeds the detached local launch `engine::launch::start`
  (`launch.rs:142`) for served `task.run`/`task.ship`, and `graph.rs:19`,
  `skill.rs:16`, `pr.rs:212`, `repo.rs:29`, `checkout.rs:238`,
  `creation.rs:156`.
- **Parses per invocation.** `bin/lf.rs` parses up to five times on the main
  path (:1548 config fast path, :1689 `--open`, `navigation::machine_invocation`,
  `ssh::parse_remote_command`, :1752), plus `preview_invocation` (:2114). About
  23 commands are intercepted before store setup. Config get/set runs at
  :1571–1632 and again at :2863–2903, the second only for
  `--machine <this machine> config …`. The `unreachable!` is at :3051–3056.
- **`Read::argv` (`read.rs:387`) forwards nothing.** Its callers:
  - the "has no --subscribe" error (:988);
  - `session_filter` (:1014), which re-parses argv to borrow Clap's defaults;
  - `tests/served_reads.rs:751,1254,1273`, which compare served output with
    CLI output.
- **Of the 23 served-flag call sites, only 6 drop env selection**
  (group A below). The rest stay:
  - Process/journal identity (`journal/mod.rs:366,421,467,606,1185`): a
    request is a trace, not an OS Process.
  - The read cache clock (`observe::active` ×3).
  - No terminal (`provider_account::operations::served`, used by
    `narrate!`/`diagnose!`, the Chrome-profile prompt and
    `manual_authorization`).
- **Swift.**
  - `RegistryQuery.swift` already sends typed operations for almost everything
    (68 `serve`/`write` calls). `ServedRead`/`ServedWrite` keep an `argv`
    beside each `encode` for the no-server fallback (:826–840).
  - `onMachine` (:911) drops `serve`/`write`, so every remote-Machine call
    becomes `lf --machine M --repo-id R <argv>`.
  - There is no codegen. `lf server describe` emits JSON Schema
    (`server/schema.rs`), and `ServerProtocolTests.swift` only checks field
    sets against it.

## The concept (concept review, 2026-11-17)

`scratch/concept-review.html` has the review; `scratch/questions.md` has every
answer word for word. What they settle:

- There is one protocol (`machine/wire.rs`, revision 5) and one executor of
  it on a Machine: its resident server. A client opens a **connection to a
  Machine's server** and sends operations.
- A connection has a **carrier**. The local socket (`machine.sock`) for this
  Machine, SSH for every other Machine, the tailnet WebSocket for the phone.
  The Mac gets no WebSocket client: with SSH a pipe to the server it would be
  a second route with no new ability.
- Over SSH, the entry on the destination is a **pipe** to that Machine's
  `machine.sock`. It runs nothing itself. It starts the server when it is not
  running.
- Work sent to another Machine therefore needs that Machine's server. This
  drops the brief's "SSH routing needs no resident API server on the
  destination". Commands run on a Machine itself still need no server, and
  `ssh <machine> lf …` typed by hand is the way in when a server cannot run.
- Named logins and per-run secrets travel with the SSH carrier. LOO-957
  redesigns what they are; this Task only keeps one place that delivers them.
- The stack stopped at PR #1956 for this redesign.

## Demo

Machine `mini` is added, and Task LOO-123 executes there.

```bash
lf task hold LOO-123 --json             # one ssh; mini's server answers
lf task status LOO-123 --subscribe      # live lines from mini's server, same route
ssh mini lf server status               # the one connected client is this laptop
```

Stop mini's server and repeat the hold: it answers after the entry starts the
server. Make the server unstartable: the hold fails naming mini, the reason
and `ssh mini lf server status`.

## Design

### One client

`machine::client::Connection` is the protocol's client, over a `UnixStream`
today. It becomes generic over its carrier's two byte streams:

```rust
Connection::open()              // machine.sock, as today
Connection::open_on(&machine)   // child `ssh … lf server connect`, piped
```

`request`, `subscribe`, `receive` and `checkpoint` are unchanged. #1956's
`machine::ssh::call` builds `hello` and one `request` by hand; it is a second
client and is deleted.

### The SSH entry is a pipe

Named `lf server connect` here; #1956 called it `lf server stdio`. On the
destination it:

1. checks `LF_EXPECTED_MACHINE_ID` before anything else, as every receiving
   invocation must (`machines.md:47–52`);
2. ensures the server is running (`installation::server::ensure`, the path a
   held launch already takes);
3. connects to `machine.sock` and copies bytes both ways until either side
   closes.

It parses nothing after step 1, so every message the protocol has or gains
works over SSH: requests, `subscribe`, `session.start`, attach. A ticketed
connection (terminal bytes, bulk messages) is a second `lf server connect
--ticket`, the same pipe to the socket's ticket entrance.

An `lf` too old for the entry exits 2, which becomes "mini runs lf
<version>; update it with …" (built in #1956). A server on an older protocol
answers `incompatible` itself.

`server.md:113` forbids forwarding the trusted local socket for the phone,
whose proxy would lend it another user's authority. An SSH login is the same
OS user the socket already trusts.

### What the server must learn

Today's SSH preamble and `--repo-id` give the destination `lf` its context.
A pipe gives the server none, so the context moves into the protocol:

In PR 1 one operation carries all of it: `machine.write { repository,
environment, forwarded, change }` (`server/wire.rs`). The server runs `change`
with `Write::run_here` in its checkout of `repository` and never sends it on;
`hello`'s kind `machine` only labels the client in `lf server status`.
`environment` is the preamble's variables (profile, user name, Task source,
agent override); `forwarded` names the one-time file `forward::deliver`
wrote, which the server reads once (`forward::take`) for the launch.

`machine.write` is a second envelope around `Write`, and `environment` puts
environment-variable names on the wire. Both last only until PR 3: there,
`Request` carries a typed `Selection` (repository, profile, participant,
Task source, agent, forwarded file), every write is plain `Write` with that
selection, and `machine.write` is deleted.

### One way to open SSH to a Machine

`machine::ssh::open(machine)` runs `ssh … lf server connect` and hands its
pipes to `Connection`. SSH multiplexes already (`machine_route.rs:46`), so
further connections cost no handshake. The plain-command launch
(`ssh::run_with_io`) remains for `install`, sign-in flows and commands with
no `Operation`. Nothing else starts `ssh`: `machine::task::run_remote`,
`Write::prepare_session`'s SSH half and `ssh::call` go in this Task;
`ssh::request` and `credentials::remote_command` go when their callers
become operations.

## Slices

Each PR is its own Task. This Task is PR 1.

### PR 1, this Task: SSH is a pipe to the destination's server

Rework #1956 on `main`:

- `Connection` over a carrier; `ssh::open`; `lf server connect`.
- `machine.write` carries the repository id, the sender's context and the
  forwarded file; the server runs its `change` here.
- Delete `ssh::call`, the request executor in `server/stdio.rs`, and every
  `prepare_launch` call (LOO-913 deleted the function).
- Kept from #1956: `Write::run` routes and `Write::run_here` runs; engine
  `remote`/`remote_on`/`RemoteTaskOperation`, `run_remote` and
  `Write::cli()` deleted; the skew, crash and unreachable errors.
- `docs/dev/machines.md:79` and `docs/dev/server.md` say the new rule: work
  sent to another Machine runs in its server.

### PR 2: conversations and live updates over it

- `lf --machine mini resume S`, Task conversations and Desktop shells on
  another Machine open with `session.attach` and a ticket over the pipe.
- The SSH relay to `held.sock` is deleted once nothing older needs it.
- `lf task status --subscribe` for a Task on another Machine.

### PR 3: Clap produces an `Invocation`; one dispatcher

As the first plan had it. `--machine` sends the operation over
`Connection::open_on`; a command with no `Operation` takes the plain-command
launch. `Selection` moves onto `Request` with typed fields, `machine.write`
and its `environment` map are deleted, and the preamble's environment goes. `mutation.rs`, CLI `work_route::forward`, `Read::argv`,
`aliases.rs`, the extra parses and the group A served guards are deleted;
`transport::delegate_current` and the detached launch send an operation;
`ssh::request`'s callers get kinds; the Clap → `Invocation` table is
snapshot-tested.

### PR 4: Swift

- `onMachine` keeps `serve`/`write` and names the Machine in the request's
  selection. The local server holds a `Connection` to that Machine, so its
  Tasks update live in Desktop.
- Delete every `argv` in `ServedRead`/`ServedWrite` and the no-server
  fallback (`RegistryQuery.swift:826–840`).

### Order with LOO-957

LOO-957 (sign-in, based on `main`, running) retires `lf account` and changes
what is delivered to a Machine. It touches the two `forward::deliver` call
sites and the CLI entry code PR 3 rewrites.

### State of the branch (2026-11-17)

PR 1 is built on `main` at `e66b21801c`: `Connection::open_on` over
`machine::ssh::Pipe`, `lf server connect` (`server/connect.rs`), and
`machine.write` (`server/wire.rs::MachineOperation`), which the server runs
with `Write::run_here` in its checkout of the named repository.
`server/stdio.rs`, `ssh::call` and the `prepare_launch` path are deleted.
A destination `lf` that exits non-zero (not 2, 255) adds "; check `ssh
<route> lf server status`" (`machine::ssh::Pipe::ended`), the demo's third
failure clause. The Done-when `rg` finds nothing.

## Open

- In-process writes sent to another Machine (hold, claim, unassign, Session
  preparation) run with the destination's own configuration; the sender's
  `environment` reaches only the launches `task.run` and `task.ship` start.
  PR 3's `Selection` on `Request` closes this.
- The SSH carrier sets no read timeout; SSH bounds connecting, not a stalled
  destination.
- PR #1956 CI reports every check SKIPPED except `macos-inputs` and
  `scratch-clear`, including `rust-test` (rechecked 2026-11-17 at
  `836125fd5a`). Not investigated; gate must see the suites run.

## Built in #1956 (2026-11-16), before the redesign

- `cargo test -p loopflow --test integration machine_commands::`: 60 pass
  after the QA fixes; `--lib server::` 87; `machine::ssh::` 4; clippy clean.
- QA (two private Machines, stand-in ssh; `scratch/qa.html`): typed
  hold/claim/unassign/run/ship reach mini; older lf, older protocol, crashing
  and unreachable mini each name the cause.
- Loopback sshd (`tests/transport/ssh_shells.rs`): CI.

## Checks (PR 1)

- `cargo nextest run -p loopflow --test integration -E 'test(served_ownership_routes_to_its_writer) | test(a_hold_for_a_task_mini_runs) | test(a_served_run_for_a_task_mini) | test(an_lf_without_the_entry) | test(an_lf_that_dies_before) | test(a_served_resume_opens_the_session_on_your)'`: 6 pass; the three SSH-entry ones rerun after the status hint, pass (mini's real server answers the hold and the claim).
- `--lib` `machine::forward:: | machine::client:: | machine::ssh:: | server::schema:: | server::wire:: | lf::commands::write:: | lf::tests::`, `documented_commands::`: pass; clippy clean on both crates.
- compress (2026-11-17): `Socket`'s far end is one `End` enum; `Write::execute` takes the `Option<Remote>` it routes to. `machine::client::` 5, `lf::commands::write::` 3, the three SSH-entry tests: pass; clippy clean.
- A stopped destination server started by the entry: QA, not automated (`ensure` registers a supervisor job).
- Swift `ServerProtocolTests` against the regenerated schema: gate/CI.

## Done when (PR 1)

- A Task or Session write for another Machine travels as a typed request on
  a `Connection` over SSH and is run by that Machine's server.
- With the destination's server stopped, the same write succeeds after the
  entry starts it.
- `rg 'prepare_launch|fn run_remote|fn remote_on|ssh::call|\.cli\(\)' rust/*/src`
  finds nothing (`fn cli(` also matches unrelated cmux and Rex helpers).
- An older destination fails with its version and the update command.
