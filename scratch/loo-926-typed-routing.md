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

The request carries it, typed (concept review of the built PR, 2026-11-17;
answers in `scratch/questions.md`):

```
hello     { client: { kind: "machine" } }
request   { id, operation: <any Operation>, select,
            sender: { repository, selection: { profile, participant, agent,
                      settings, source }, secrets: { NAME: value } } }
subscribe { …, resume, sender }
```

- **The connection says where it runs.** A client whose `hello` kind is
  `machine` has every operation run here, in this Machine's checkout of
  `repository`, and never sent on: `Write::run_here`, and reads and
  subscriptions through the same rule. The operation's kind decides nothing.
- **`sender` is a field of `request` and `subscribe`**, required and
  nullable; a local client sends null. `selection` is the
  sender's profile, participant name, agent override and Task source as their
  own types, not environment-variable names.
- **`secrets` are the run's forwarded values, in the request.** It is on SSH
  stdin and ends at an owner-only socket. The field's type prints nothing
  (`Debug`, request telemetry, the journal, `lf server status`), and a test
  holds that. Files named by `forward.files` are still delivered as files by
  `forward::deliver`. If no type can make the telemetry claim certain, keep
  the one-time file and say so in `scratch/questions.md`.
- **Deleted from the built branch:** `MachineOperation` and `machine.write`,
  the `environment` map, `forwarded`, `Write::in_checkout`'s JSON rewrite,
  `forward::take` and `forward::exported`.

Every client encodes `Request`, so this changes Desktop's and the phone's
encoder and the DTO fixtures in the same PR (two null fields). The headless
choice it replaces picked a new operation to avoid that; the wrapper could
carry only writes.

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
- `Request` carries `repository`, a typed `selection` and `secrets`; a
  `machine` connection's operations run here. `machine.write` is deleted
  before it ships.
- Reads and `--subscribe` for a Task another Machine runs go over the same
  connection: `lf task status LOO-123 --subscribe` prints mini's live lines.
- The SSH carrier bounds a wait for an answer to a request (not a
  subscription's quiet stream); past it the command fails naming the Machine
  and `ssh <route> lf server status`.
- `Write::TaskHold` gains `reason`; the CLI's hold calls `Write::run` and the
  routing copy in `mutation.rs` goes.
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

### PR 3: Clap produces an `Invocation`; one dispatcher

As the first plan had it. `--machine` sends the operation over
`Connection::open_on`; a command with no `Operation` takes the plain-command
launch. The preamble's environment goes (`Selection` is on `Request` since PR 1). `mutation.rs`, CLI `work_route::forward`, `Read::argv`,
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

Built on `main` at `e66b21801c`: `Connection::open_on` over
`machine::ssh::Pipe`, `lf server connect` (`server/connect.rs`), the skew,
crash, unreachable and unstartable errors, and "What the server must learn":

- `machine/wire.rs`: `Sender`, `Selection`, `Secrets` (`Debug` names only),
  `MACHINE_CLIENT`; `sender` on `request` and `subscribe`; DTO fixtures,
  Swift's encoder and `server_schema.json` carry it.
- `server/service.rs`: `Origin` from the `hello` kind; a Machine's writes
  `Write::run_here(sender)`, its reads and subscriptions `Read::directory_mut`
  into the sender's checkout.
- `Selection::capture` / `environment` replace `forwarding_environment`, also
  for the plain-command launch in `lf/commands/ssh.rs`.
- `Read::TaskStatus` (`task.status`); `lf task status <issue> --subscribe`
  for a Task another Machine runs subscribes there
  (`waves::subscribe_remote_task_status`).
- A request over SSH waits 120 s (`client::ANSWER`, a timed pipe reader),
  then fails `outcome_unknown` naming the Machine and `ssh <route> lf server
  status`; the SSH handshake shares that bound.
- `Write::TaskHold` has `reason`; the CLI hold runs `Write::run`.
- Deleted: `MachineOperation`/`machine.write`, `Write::in_checkout`,
  `forward::take`/`exported`, `forwarding_environment`.

Merged `origin/main` at `2593c489d7` (#1960): `request_id` on `request`.

Then, to finish PR 1's concept review of `4be26c7113`:

- Protocol revision 8, `MIN_REVISION` 8 (`machine/wire.rs`,
  `ServerProtocol.swift`, `server/` DTO fixtures, `server_schema.json`,
  server README): Desktop and lf from main speak 7 without `sender` and now
  answer `incompatible` at `hello`.
- `Origin::Machine(Box<Sender>)`; a Machine's null `sender` is `invalid`
  (`a_machine_request_without_its_sender_is_invalid`); `run_here` takes
  `&Sender`.
- Secrets stay out of `Debug` (`server::wire::tests::a_request_another_machine_sent_…`)
  and out of `logs/lf.log` for a served Machine request
  (`a_machine_request_writes_its_secrets_nowhere_in_lf_log`).

Checks: `cargo test -p loopflow --lib server::` passed (94). Swift
`scripts/test_desktop.sh --filter 'DTOFixtureTests|ServerClientTests'` passed (74).

Kept on purpose: the plain-command launch (`lf/commands/ssh.rs`) still sends
`forward.env` as a delivered one-time file, because no server is reached.
It is the only remaining env-file path.

Not done here:
- `Write::remote_session` builds a Clap command line (`session_spelling`) to
  ask `work_route::forward` which Machine. PR 3.
- `ContinueRun` is passed through `move_session`/`move_task` because `Write`
  lives in the application crate. Larger than this PR.

## Open

- No CI suite has run on this branch. #1956 carries `scratch/`, so
  `scratch-clear` defers the matrix (`.github/CI.md`, "Scratch, cancellation
  and macOS path filters"); gate clears scratch and the suites run then.

## Built in #1956 (2026-11-16), before the redesign

- `cargo test -p loopflow --test integration machine_commands::`: 60 pass
  after the QA fixes; `--lib server::` 87; `machine::ssh::` 4; clippy clean.
- QA (two private Machines, stand-in ssh; `scratch/qa.html`): typed
  hold/claim/unassign/run/ship reach mini; older lf, older protocol, crashing
  and unreachable mini each name the cause.
- Loopback sshd (`tests/transport/ssh_shells.rs`): CI.

## Checks (PR 1)

- `--lib` `server::wire:: | server::schema:: | machine:: | lf::commands::write:: | lf::commands::read:: | server::service:: | server::remote:: | lf::tests::`: 106 pass.
- `--test integration` the six SSH-entry/served tests, `an_unreachable_destination_names_the_ssh_check_not_its_server`, `a_status_subscription_for_a_task_mini_runs_reads_minis_server`, `documented_commands::`: pass.
- `--test served_reads server_operations_match_their_lf_commands` (covers `task.status`): pass.
- clippy `-p loopflow -p loopflow-execution --all-targets -D warnings`: clean.
- The 120 s answer bound: unit test of the timed pipe reader only; a silent destination end to end is QA.
- A stopped destination server started by the entry: QA (`ensure` registers a supervisor job); `scratch/qa.html` from before the `sender` rework.
- Swift `ServerProtocolTests`/`RegistryQueryTests` (`sender: null`, `task.hold` `reason`): gate/CI.

## Done when (PR 1)

- A Task or Session write for another Machine travels as a typed request on
  a `Connection` over SSH and is run by that Machine's server.
- With the destination's server stopped, the same write succeeds after the
  entry starts it.
- `rg 'prepare_launch|fn run_remote|fn remote_on|ssh::call|\.cli\(\)' rust/*/src`
  finds nothing (`fn cli(` also matches unrelated cmux and Rex helpers).
- An older destination fails with its version and the update command.
- `lf task status <issue> --subscribe` for a Task another Machine runs prints
  that Machine's live lines.
- A destination that accepts SSH and never answers fails within the bound,
  naming the Machine.
- `rg 'machine\.write|MachineOperation|in_checkout|forward::take' rust/*/src`
  finds nothing.
